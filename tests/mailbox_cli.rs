#[allow(dead_code)]
#[path = "../src/collab/store.rs"]
mod store;
use store::*;
use std::{fs, process::Command};

#[test]
fn independent_agent_processes_exchange_replies_without_overwriting() {
    let root = std::env::temp_dir().join(Store::new_id("cli-test"));
    let db = Store::new(&root).unwrap();
    db.create_round(Round {
        id: "test".into(), title: "Test".into(), goal: "Exchange".into(), repo: root.clone(), created: Store::now(), phase: Phase::Development,
        participants: vec![Participant { id: "a".into(), name: "A".into(), role: "Developer".into() }, Participant { id: "b".into(), name: "B".into(), role: "Reviewer".into() }],
    }).unwrap();
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_clihub-agent")).args(args).arg("--room").arg(&root).output().unwrap();
        assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
        String::from_utf8(output.stdout).unwrap()
    };
    let first = run(&["send", "--from", "a", "--to", "b", "--body", "Review this"]);
    let second = run(&["send", "--from", "b", "--to", "a", "--body", "Changes requested", "--reply-to", first.trim()]);
    assert_ne!(first, second);
    let inbox = run(&["inbox", "--agent", "a"]);
    let reply: Message = serde_json::from_str(inbox.trim()).unwrap();
    assert_eq!(reply.body, "Changes requested");
    assert_eq!(reply.reply_to.as_deref(), Some(first.trim()));
    run(&["read", "--agent", "a", "--message", second.trim()]);
    assert!(run(&["inbox", "--agent", "a", "--unread"]).trim().is_empty());
    assert_eq!(db.all_messages().unwrap().len(), 2);
    let duplicate = db.read(first.trim()).unwrap();
    assert!(db.send(duplicate).is_err());
    assert!(db.read("../round").is_err());
    assert!(db.mark_read("a", first.trim()).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn simultaneous_senders_publish_only_complete_unique_messages() {
    let root = std::env::temp_dir().join(Store::new_id("concurrent-test"));
    let db = Store::new(&root).unwrap();
    db.create_round(Round { id: "test".into(), title: "Test".into(), goal: "Concurrent".into(), repo: root.clone(), created: Store::now(), phase: Phase::Development, participants: vec![Participant { id: "a".into(), name: "A".into(), role: "Developer".into() }] }).unwrap();
    let threads: Vec<_> = (0..12).map(|_| {
        let dir = root.clone();
        std::thread::spawn(move || {
            Store::new(dir).unwrap().send(Message { id: Store::new_id("msg"), round: "test".into(), from: "a".into(), to: "a".into(), body: "x".repeat(10000), reply_to: None, anchor: None, snapshot: None, time: Store::now() }).unwrap();
        })
    }).collect();
    while threads.iter().any(|t| !t.is_finished()) { db.all_messages().unwrap(); std::thread::yield_now(); }
    for thread in threads { thread.join().unwrap(); }
    assert_eq!(db.all_messages().unwrap().len(), 12);
    fs::remove_dir_all(root).unwrap();
}
