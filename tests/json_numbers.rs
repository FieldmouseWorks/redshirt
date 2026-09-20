use redshirt::evidence::{decode, encoded};
use serde_json::json;
use std::{fs, process::Command};

#[test]
fn finite_binary64_matches_independent_python_json_oracle() {
    let mut numbers = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        0.2,
        1e-4,
        1e-5,
        1e15,
        1e16,
        1e20,
        1e21,
        f64::MIN_POSITIVE,
        f64::MAX,
        f64::from_bits(1),
        1.2345678901234567,
    ];
    for edge in [1e-4_f64, 1e16_f64] {
        numbers.extend([
            f64::from_bits(edge.to_bits() - 1),
            f64::from_bits(edge.to_bits() + 1),
        ]);
    }
    let mut seed = 0x5eed_cafe_1234_5678_u64;
    for _ in 0..16384 {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        let n = f64::from_bits(seed);
        if n.is_finite() {
            numbers.push(n);
        }
    }
    let value = json!({"floats":numbers,"integer":1,"text":"é😀\u{7f}"});
    let encoded = encoded(&value).unwrap();
    let tmp = tempfile::tempdir().unwrap();
    let path = tmp.path().join("numbers.json");
    fs::write(&path, &encoded).unwrap();
    let result = Command::new("python3").args(["-c",
        "import json,sys; v=json.load(open(sys.argv[1])); sys.stdout.buffer.write(json.dumps(v,sort_keys=True,separators=(',',':'),allow_nan=False).encode())"])
        .arg(path).output().expect("Python baseline required for numeric contract");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    // Report only the first differing number, avoiding an enormous assertion dump.
    let expected = decode(&result.stdout).unwrap();
    for (i, (actual, oracle)) in value["floats"]
        .as_array()
        .unwrap()
        .iter()
        .zip(expected["floats"].as_array().unwrap())
        .enumerate()
    {
        assert_eq!(
            actual.as_f64().unwrap().to_bits(),
            oracle.as_f64().unwrap().to_bits(),
            "float {i}"
        );
    }
    let mismatch = encoded.iter().zip(&result.stdout).position(|(a, b)| a != b);
    assert!(
        mismatch.is_none() && encoded.len() == result.stdout.len(),
        "byte mismatch at {mismatch:?}"
    );
}
