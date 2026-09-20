use redshirt::{
    Provider,
    interaction::{Interactive, completion},
};
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

#[tokio::test]
async fn cancelled_partial_write_keeps_receipt_and_finishes_valid_frames() {
    let (writer, reader) = tokio::io::duplex(16);
    let mut provider = Interactive::new(tokio::io::empty(), writer);
    let request = json!({"version":1,"observation":{"counter":0},
        "candidates":{"stop":"Stop."},"remaining_inputs":24});
    assert!(
        tokio::time::timeout(Duration::from_millis(10), provider.select(request))
            .await
            .is_err()
    );
    let receipts = provider.take_evidence();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0]["status"], "interrupted");
    let report = json!({"stop":"cancelled","requests":1,"attempted_inputs":0,
        "final":{"ok":true},"cleanup":true,"replayable":true});
    let expected = completion(&report);
    let finish = tokio::spawn(async move { provider.finish(&report).await });
    let mut reader = BufReader::new(reader);
    for kind in ["observation", "done"] {
        let mut line = String::new();
        tokio::time::timeout(Duration::from_secs(1), reader.read_line(&mut line))
            .await
            .unwrap()
            .unwrap();
        let frame: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(frame["type"], kind);
        if kind == "done" {
            assert_eq!(frame, expected);
        } else {
            assert_eq!(frame["decision_id"], receipts[0]["decision_id"]);
        }
    }
    finish.await.unwrap().unwrap();
}
