// Extends the original targeted GC owner and its existing isolated database.
fn assert_reserved_candidate_lifecycle(database: &mut Database, store: &ManagedBlobStore) {
    let bytes = b"reserved outside-lock task candidate";
    let reservation = store
        .reserve_candidate(
            database,
            bytes,
            "text/plain",
            "sensitive",
            GC_OWNER_EXECUTION_LIFECYCLE,
        )
        .unwrap();
    let prepared = std::thread::spawn(move || reservation.write(bytes))
        .join()
        .unwrap()
        .unwrap();
    let id = prepared.id().to_string();
    let digest = format!("{:x}", Sha256::digest(bytes));
    let path = store.root.join(blob_relative_path(&digest));
    let future = "9999-01-01T00:00:00Z";
    assert!(path.is_file());
    assert!(
        load_blob_metadata(database.connection(), &id)
            .unwrap()
            .is_none()
    );
    store.recover_gc_state(database).unwrap();
    assert!(
        path.is_file(),
        "active unregistered candidate must survive recovery"
    );
    let tx = database.connection_mut().transaction().unwrap();
    prepared.register(&tx).unwrap();
    tx.rollback().unwrap();
    assert!(
        load_blob_metadata(database.connection(), &id)
            .unwrap()
            .is_none()
    );
    assert!(
        !store
            .collect_gc_candidates_before(database, future, 100)
            .unwrap()
            .contains(&id)
    );
    let tx = database.connection_mut().transaction().unwrap();
    prepared.register(&tx).unwrap();
    tx.commit().unwrap();
    assert!(
        !store
            .collect_unreferenced_before(database, future)
            .unwrap()
            .contains(&id)
    );
    assert!(
        !store
            .collect_gc_candidates_before(database, future, 100)
            .unwrap()
            .contains(&id)
    );
    assert!(path.is_file());
    drop(prepared);
    assert!(
        store
            .collect_gc_candidates_before(database, future, 100)
            .unwrap()
            .contains(&id)
    );
    assert!(!path.exists());

    let abandoned = store
        .reserve_candidate(
            database,
            b"abandoned task",
            "text/plain",
            "sensitive",
            GC_OWNER_EXECUTION_LIFECYCLE,
        )
        .unwrap()
        .write(b"abandoned task")
        .unwrap();
    let id = abandoned.id().to_owned();
    let path = store.root.join(blob_relative_path(&format!(
        "{:x}",
        Sha256::digest(b"abandoned task")
    )));
    drop(abandoned);
    store.recover_gc_state(database).unwrap();
    assert!(!path.exists());
    assert!(
        load_blob_metadata(database.connection(), &id)
            .unwrap()
            .is_none()
    );

    let bytes = b"existing candidate protected before write";
    let old = store
        .put_bytes_candidate(
            database,
            bytes,
            "text/plain",
            "sensitive",
            GC_OWNER_EXECUTION_LIFECYCLE,
        )
        .unwrap();
    let reservation = store
        .reserve_candidate(
            database,
            bytes,
            "text/plain",
            "sensitive",
            GC_OWNER_EXECUTION_LIFECYCLE,
        )
        .unwrap();
    assert!(
        !store
            .collect_gc_candidates_before(database, future, 100)
            .unwrap()
            .contains(&old.id)
    );
    assert_eq!(store.read_bytes(database, &old.id).unwrap(), bytes);
    drop(reservation);
    assert!(
        store
            .collect_gc_candidates_before(database, future, 100)
            .unwrap()
            .contains(&old.id)
    );

    let reservation = store
        .reserve_candidate(
            database,
            b"expected original",
            "text/plain",
            "sensitive",
            GC_OWNER_EXECUTION_LIFECYCLE,
        )
        .unwrap();
    assert!(reservation.write(b"different input").is_err());
    store.recover_gc_state(database).unwrap();

    // Two independent workers can publish identical content; dropping one must
    // not expose the shared address while the other still owns its reservation.
    let bytes = b"concurrent identical task body";
    let left = store
        .reserve_candidate(
            database,
            bytes,
            "text/plain",
            "sensitive",
            GC_OWNER_EXECUTION_LIFECYCLE,
        )
        .unwrap();
    let right = store
        .reserve_candidate(
            database,
            bytes,
            "text/plain",
            "sensitive",
            GC_OWNER_EXECUTION_LIFECYCLE,
        )
        .unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
    let left_barrier = barrier.clone();
    let left = std::thread::spawn(move || {
        left_barrier.wait();
        left.write(bytes)
    });
    let right = std::thread::spawn(move || {
        barrier.wait();
        right.write(bytes)
    });
    let left = left.join().unwrap().unwrap();
    let right = right.join().unwrap().unwrap();
    assert_eq!(left.id(), right.id());
    let id = right.id().to_owned();
    let path = store
        .root
        .join(blob_relative_path(&format!("{:x}", Sha256::digest(bytes))));
    drop(left);
    store.recover_gc_state(database).unwrap();
    assert!(path.is_file());
    let tx = database.connection_mut().transaction().unwrap();
    right.register(&tx).unwrap();
    tx.commit().unwrap();
    right.finish().unwrap();
    drop(right);
    assert!(
        store
            .collect_gc_candidates_before(database, future, 100)
            .unwrap()
            .contains(&id)
    );
    assert!(!path.exists());
}
