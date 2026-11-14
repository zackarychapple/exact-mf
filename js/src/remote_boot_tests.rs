fn boot_record(dir: &Directory) -> serde_json::Value {
    serde_json::from_slice(&std::fs::read(dir.0.join("record.json")).unwrap()).unwrap()
}

fn select_for_boot(owner: &mut RemoteCoordinator, seq: u64, good: bool) -> RemoteHandle {
    let selected = stage(owner, seq);
    let token = owner
        .prepare(&selected.generation, "remote.hbc", "test.pure", "", &source_plan())
        .unwrap();
    let handle = owner.activate_after_first_pixel(&token).unwrap();
    owner.commit(&token).unwrap();
    if good {
        owner.boot_succeeded(&handle).unwrap();
    }
    handle
}

#[test]
fn selected_failures_count_before_execution_and_demote_at_open() {
    let dir = Directory::new();
    let mut owner = RemoteCoordinator::new(dir.store());
    select_for_boot(&mut owner, 1, true);
    select_for_boot(&mut owner, 2, false);
    let initial = boot_record(&dir);
    drop(owner);
    let mut stale = Vec::new();
    for count in [1, 2] {
        let mut owner = RemoteCoordinator::new(dir.store());
        let attempt = owner.begin_selected_boot().unwrap().unwrap();
        assert_eq!(attempt.generation().seq, 2);
        assert_eq!(boot_record(&dir)["failures"], count);
        let selected = owner.prepare_selected_boot(&attempt).unwrap();
        assert_eq!(selected.generation, *attempt.generation());
        if count == 2 {
            let token = owner
                .prepare_selected_remote(&attempt, "remote.hbc", "wrong.identity", "", &source_plan())
                .unwrap();
            assert!(owner.activate_after_first_pixel(&token).is_err());
        }
        owner.fail_selected_boot(&attempt).unwrap();
        assert_eq!(owner.store().generation().entry, None);
        let record = boot_record(&dir);
        assert_eq!(record["selected"], initial["selected"]);
        assert_eq!(record["lastGood"], initial["lastGood"]);
        assert_eq!(record["stream"]["seq"], 2);
        assert_eq!(record["failures"], count);
        assert!(owner.refuse_selected_boot(&attempt, "late corruption").is_err());
        assert_eq!(boot_record(&dir), record);
        stale.push(attempt);
    }
    let mut owner = RemoteCoordinator::new(dir.store());
    assert_eq!(owner.store().generation().seq, 1);
    let record = boot_record(&dir);
    assert_eq!(record["selected"], initial["lastGood"]);
    assert_eq!(record["bad"][0], initial["selected"]);
    assert_eq!(record["stream"]["seq"], 2);
    for attempt in stale {
        assert!(owner.fail_selected_boot(&attempt).is_err());
        assert!(owner.refuse_selected_boot(&attempt, "stale").is_err());
    }
    let attempt = owner.begin_selected_boot().unwrap().unwrap();
    let token = owner
        .prepare_selected_remote(&attempt, "remote.hbc", "test.pure", "", &source_plan())
        .unwrap();
    let handle = owner.activate_after_first_pixel(&token).unwrap();
    owner.commit(&token).unwrap();
    owner.boot_started(&handle).unwrap();
    assert_eq!(boot_record(&dir)["failures"], 1, "the attempt counted this boot once");
    owner.boot_succeeded(&handle).unwrap();
    assert!(owner.fail_selected_boot(&attempt).is_err());
    assert_eq!(boot_record(&dir)["failures"], 0);
    let old = download(&owner, 1);
    assert!(owner.finish_check(old).unwrap_err().contains("below the accepted seq 2"));
}

#[test]
fn selected_boot_ignores_pending_and_cannot_clear_its_later_commit() {
    let dir = Directory::new();
    let mut owner = RemoteCoordinator::new(dir.store());
    select_for_boot(&mut owner, 1, true);
    let newer = stage(&mut owner, 2);
    drop(owner);
    let mut owner = RemoteCoordinator::new(dir.store());
    let attempt = owner.begin_selected_boot().unwrap().unwrap();
    assert_eq!(attempt.generation().seq, 1);
    let selected = owner.prepare_selected_boot(&attempt).unwrap();
    assert_eq!(selected.generation.seq, 1);
    assert_eq!(owner.store().prepare_activation().unwrap().unwrap().generation.seq, 2);
    assert!(owner.prepare(&newer.generation, "remote.hbc", "test.pure", "", &source_plan()).is_err());
    let token = owner
        .prepare_selected_remote(&attempt, "remote.hbc", "test.pure", "", &source_plan())
        .unwrap();
    let old = owner.activate_after_first_pixel(&token).unwrap();
    owner.commit(&token).unwrap();
    assert!(owner.prepare_selected_remote(&attempt, "remote.hbc", "test.pure", "", &source_plan()).is_err());
    owner.boot_succeeded(&old).unwrap();
    assert_eq!(boot_record(&dir)["pending"], newer.generation.entry.clone().unwrap());
    let token = owner.prepare(&newer.generation, "remote.hbc", "test.pure", "", &source_plan()).unwrap();
    let current = owner.activate_after_first_pixel(&token).unwrap();
    owner.commit(&token).unwrap();
    let before = boot_record(&dir);
    assert!(owner.fail_selected_boot(&attempt).is_err());
    assert!(owner.refuse_selected_boot(&attempt, "old selected bytes").is_err());
    assert!(owner.boot_succeeded(&old).is_err());
    assert_eq!(boot_record(&dir), before);
    owner.boot_succeeded(&current).unwrap();
    assert_eq!(boot_record(&dir)["lastGood"], newer.generation.entry.unwrap());
}

#[test]
fn corrupt_selected_plan_and_assets_clear_roles_without_counting_execution_failure() {
    for asset in [false, true] {
        let dir = Directory::new();
        let mut owner = RemoteCoordinator::new(dir.store());
        select_for_boot(&mut owner, 1, true);
        let newer = stage(&mut owner, 2);
        drop(owner);
        let mut owner = RemoteCoordinator::new(dir.store());
        let attempt = owner.begin_selected_boot().unwrap().unwrap();
        let path = if asset {
            let selected = owner.prepare_selected_boot(&attempt).unwrap();
            dir.0.join("entries").join(selected.generation.entry.unwrap()).join("assets/remote.hbc")
        } else {
            owner.store().select().plan.unwrap()
        };
        std::fs::remove_file(&path).unwrap();
        std::fs::write(path, b"corrupt authenticated bytes").unwrap();
        if asset {
            assert!(owner.prepare_selected_remote(&attempt, "remote.hbc", "test.pure", "", &source_plan()).is_err());
        } else {
            assert!(owner.prepare_selected_boot(&attempt).is_err());
        }
        let record = boot_record(&dir);
        assert_eq!(record["selected"], serde_json::Value::Null);
        assert_eq!(record["lastGood"], serde_json::Value::Null);
        assert_eq!(record["failures"], 0);
        assert_eq!(record["pending"], newer.generation.entry.unwrap());
        assert_eq!(record["stream"]["seq"], 2);
        assert!(owner.fail_selected_boot(&attempt).is_err());
        assert!(owner.refuse_selected_boot(&attempt, "replay").is_err());
        assert_eq!(boot_record(&dir), record);
        assert!(owner.begin_selected_boot().unwrap().is_none());
    }
}

#[test]
fn routing_refusal_and_foreign_attempt_preserve_the_recorded_failure() {
    let a = Directory::new();
    let b = Directory::new();
    for dir in [&a, &b] {
        let mut owner = RemoteCoordinator::new(dir.store());
        select_for_boot(&mut owner, 1, true);
    }
    let mut left = RemoteCoordinator::new(a.store());
    let mut right = RemoteCoordinator::new(b.store());
    let attempt = left.begin_selected_boot().unwrap().unwrap();
    let foreign = right.begin_selected_boot().unwrap().unwrap();
    let before = boot_record(&a);
    assert!(left.prepare_selected_boot(&foreign).is_err());
    assert!(left.fail_selected_boot(&foreign).is_err());
    assert!(left.refuse_selected_boot(&foreign, "foreign").is_err());
    assert_eq!(boot_record(&a), before);
    let token = left.prepare_selected_remote(&attempt, "remote.hbc", "test.pure", "", &source_plan()).unwrap();
    let handle = left.activate_after_first_pixel(&token).unwrap();
    assert!(left.query(&handle, "base64", &[]).is_ok());
    // The host refused MF resolution before accepting this selected execution.
    left.fail_selected_boot(&attempt).unwrap();
    assert!(left.query(&handle, "base64", &[]).is_err());
    assert!(left.commit(&token).is_err());
    assert!(left.boot_succeeded(&handle).is_err());
    assert_eq!(boot_record(&a), before);
    right.fail_selected_boot(&foreign).unwrap();
    assert_eq!(boot_record(&b)["failures"], 1);
}

#[test]
fn refused_boot_mark_issues_no_attempt_and_does_not_advance_memory() {
    let dir = Directory::new();
    let mut owner = RemoteCoordinator::new(dir.store());
    select_for_boot(&mut owner, 1, true);
    drop(owner);
    let mut owner = RemoteCoordinator::new(dir.store());
    let before = boot_record(&dir);
    let record = dir.0.join("record.json");
    let saved = dir.0.join("saved-record");
    std::fs::rename(&record, &saved).unwrap();
    std::fs::create_dir(&record).unwrap();
    let error = owner.begin_selected_boot().err().unwrap();
    assert!(error.contains("cannot put"), "{error}");
    assert!(owner.selected_boot.is_none());
    std::fs::remove_dir(&record).unwrap();
    std::fs::rename(saved, record).unwrap();
    assert_eq!(boot_record(&dir), before);
    let attempt = owner.begin_selected_boot().unwrap().unwrap();
    assert_eq!(boot_record(&dir)["failures"], 1);
    owner.fail_selected_boot(&attempt).unwrap();
}

#[test]
fn revoked_selected_attempt_cannot_bless_or_recover_another_incarnation() {
    let dir = Directory::new();
    let mut owner = RemoteCoordinator::new(dir.store());
    select_for_boot(&mut owner, 1, true);
    drop(owner);
    let mut owner = RemoteCoordinator::new(dir.store());
    let attempt = owner.begin_selected_boot().unwrap().unwrap();
    let token = owner.prepare_selected_remote(&attempt, "remote.hbc", "test.pure", "", &source_plan()).unwrap();
    let handle = owner.activate_after_first_pixel(&token).unwrap();
    owner.commit(&token).unwrap();
    owner.revoke(attempt.generation());
    let before = boot_record(&dir);
    assert!(owner.boot_succeeded(&handle).is_err());
    assert!(owner.fail_selected_boot(&attempt).is_err());
    assert!(owner.refuse_selected_boot(&attempt, "late").is_err());
    assert_eq!(boot_record(&dir), before);
}

#[test]
fn refused_success_mark_preserves_the_attempt_until_a_successful_retry() {
    let dir = Directory::new();
    let mut owner = RemoteCoordinator::new(dir.store());
    select_for_boot(&mut owner, 1, true);
    drop(owner);
    let mut owner = RemoteCoordinator::new(dir.store());
    let attempt = owner.begin_selected_boot().unwrap().unwrap();
    let token = owner
        .prepare_selected_remote(&attempt, "remote.hbc", "test.pure", "", &source_plan())
        .unwrap();
    let handle = owner.activate_after_first_pixel(&token).unwrap();
    owner.commit(&token).unwrap();
    let before = boot_record(&dir);
    let record = dir.0.join("record.json");
    let saved = dir.0.join("saved-record");
    std::fs::rename(&record, &saved).unwrap();
    std::fs::create_dir(&record).unwrap();
    assert!(owner.boot_succeeded(&handle).is_err());
    assert!(owner.selected_boot.is_some());
    std::fs::remove_dir(&record).unwrap();
    std::fs::rename(saved, record).unwrap();
    assert_eq!(boot_record(&dir), before);
    owner.boot_succeeded(&handle).unwrap();
    assert!(owner.selected_boot.is_none());
    assert_eq!(boot_record(&dir)["failures"], 0);
    assert!(owner.fail_selected_boot(&attempt).is_err());
}
