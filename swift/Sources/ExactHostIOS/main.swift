import UIKit
import ExactKit
import ExactComposition
@_silgen_name("joined_call") func joinedCall(_ op: UInt32) -> UnsafePointer<CChar>
@_silgen_name("joined_finish") func joinedFinish(_ token: UInt64, _ commit: UInt32) -> Bool
@_silgen_name("joined_response_token") func joinedResponseToken() -> UInt64
let exact = ExactComposition.app
@_silgen_name("joined_configure") func joinedConfigure(_ storage: UnsafePointer<CChar>, _ resources: UnsafePointer<CChar>)
let evidence = FileManager.default.urls(for:.applicationSupportDirectory,in:.userDomainMask)[0].appendingPathComponent("ExactMF").path
let sessionIdentity = UUID().uuidString

func log(_ message: String) {
 let line = "t=\(Date().timeIntervalSince1970) pid=\(ProcessInfo.processInfo.processIdentifier) \(message)"
 NSLog("MFDEMO %@", line)
 let path = evidence + "/ui.log"
 if !FileManager.default.fileExists(atPath:path) { FileManager.default.createFile(atPath:path,contents:nil) }
 if let file = FileHandle(forWritingAtPath:path) { file.seekToEndOfFile(); file.write(Data((line + "\n").utf8)); try? file.close() }
}
final class Root: UIViewController {


 var b: ExactSession!
 let status = UILabel()
 var busy = false
 var hasRelease = false
 var timer: Timer?
 var snapshotIndex = 0
 override func viewDidLoad() {
  super.viewDidLoad()
  view.backgroundColor = .systemBackground
  try! FileManager.default.createDirectory(atPath:evidence,withIntermediateDirectories:true)
  evidence.withCString { storage in Bundle.main.resourcePath!.withCString { joinedConfigure(storage,$0) } }
  b = exact.makeSession(label:"remote")
  let stack = UIStackView(); stack.axis = .vertical; stack.spacing = 12
  let title = UILabel(); title.text = "Exact · Live Remote"; title.font = .preferredFont(forTextStyle:.title1)
  stack.addArrangedSubview(title)
  status.text = "Waiting for first draw"; status.numberOfLines = 0
  status.font = .preferredFont(forTextStyle:.footnote); status.textColor = .secondaryLabel

  for session in [b!] {
   let exactView = ExactView(session:session); exactView.clipsToBounds = true; stack.addArrangedSubview(exactView)
   exactView.heightAnchor.constraint(equalToConstant:490).isActive = true
  }
  stack.addArrangedSubview(status)
  status.font = .monospacedDigitSystemFont(ofSize:11,weight:.regular)
  stack.translatesAutoresizingMaskIntoConstraints = false; view.addSubview(stack)
  NSLayoutConstraint.activate([
   stack.leadingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.leadingAnchor,constant:16),
   stack.trailingAnchor.constraint(equalTo:view.safeAreaLayoutGuide.trailingAnchor,constant:-16),
   stack.topAnchor.constraint(equalTo:view.safeAreaLayoutGuide.topAnchor,constant:16)])
  log("launched")
  timer = Timer.scheduledTimer(withTimeInterval:2,repeats:true) { [weak self] _ in self?.poll() }
 }
 func poll() {
  guard !busy, let firstB = b.firstDrawMs else { return }
  busy = true
  log("poll firstDraw remote=\(firstB)")
  DispatchQueue.global(qos:.utility).async {
   let data = Data(String(cString:joinedCall(0)).utf8)
   let token = joinedResponseToken()
   DispatchQueue.main.async { self.complete(data,token) }
  }
 }
 func complete(_ data: Data,_ responseToken: UInt64) {
  defer { busy = false }
  var accepted = false
  defer { if !accepted && responseToken != 0 { log("abandon token=\(responseToken) result=\(joinedFinish(responseToken,0))") } }
  let object = (try? JSONSerialization.jsonObject(with:data)) as? [String:Any] ?? [:]
  if let error = object["error"] { if !hasRelease { status.text = "Connection check failed · retrying" }; log("check failed: \(error)"); return }
  if object["current"] as? Bool == true { return }
  guard let bytes = object["plan"] as? [UInt8], let assets = object["assets"] as? [String:[UInt8]], let handle = object["handle"] as? String, let token = object["token"] as? UInt64, token == responseToken else { status.text = "Invalid update · retrying"; log("invalid descriptor"); return }
  let before = ["remote":b.agent("{\"op\":\"state\"}")]
  let resolver = AssetResolver(root:exact.assetRoot,names:Array(assets.keys),generation:handle,read:{assets[$0].map{Data($0)}})
  accepted = exact.applyGeneration(ExactGeneration(plan:Data(bytes),assets:resolver),label:handle,commit:{joinedFinish(token,1)})
  if accepted { hasRelease = true }
  if accepted, let release = object["release"] as? [String:Any] {
   status.text = "\(release["name"] ?? "") · \(release["version"] ?? "")\nVerified sequence \(object["seq"] ?? "?") · entry \(String(describing:object["entry"] ?? "").prefix(10))\nPlan \(String(describing:object["planDigest"] ?? "").prefix(10)) · session \(sessionIdentity.prefix(8))"
  } else { status.text = "Update rejected · retrying" }
  log("mount seq=\(object["seq"] ?? "?") execution=\(handle) accepted=\(accepted)")
  snapshotIndex += 1
  let state: [String:Any] = ["session":sessionIdentity,"release":object["release"] ?? [:],"entry":object["entry"] ?? "","planDigest":object["planDigest"] ?? "","pid":ProcessInfo.processInfo.processIdentifier,"seq":object["seq"] ?? "?","execution":handle,"accepted":accepted,"before":before,"after":["remote":b.agent("{\"op\":\"state\"}")]]
  if let bytes = try? JSONSerialization.data(withJSONObject:state,options:.prettyPrinted) { try? bytes.write(to:URL(fileURLWithPath:evidence + "/mount-\(snapshotIndex).json")) }
 }
}
final class Delegate: UIResponder, UIApplicationDelegate {
 var window: UIWindow?
 func application(_ application:UIApplication,didFinishLaunchingWithOptions options:[UIApplication.LaunchOptionsKey:Any]?) -> Bool {
  let window = UIWindow(frame:UIScreen.main.bounds); window.rootViewController = Root(); window.makeKeyAndVisible(); self.window = window; return true
 }
}
UIApplicationMain(CommandLine.argc,CommandLine.unsafeArgv,nil,NSStringFromClass(Delegate.self))
