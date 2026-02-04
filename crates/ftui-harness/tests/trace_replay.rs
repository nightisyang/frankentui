use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

use ftui_harness::determinism::{JsonValue, TestJsonlLogger};
use ftui_harness::trace_replay::replay_trace;

const FNV_OFFSET_BASIS: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x100000001b3;
const EXPECTED_CHECKSUM_EMPTY_2X2: u64 = 0xc815b2ba593b90f5;
const EXPECTED_CHECKSUM_A_ONLY_2X2: u64 = 0x7960ba558452e6b4;
const EXPECTED_CHECKSUM_AB_2X2: u64 = 0x28f1067816e37544;

#[derive(Clone)]
struct CellData {
    kind: u8,
    char_code: u32,
    grapheme: Vec<u8>,
    fg: u32,
    bg: u32,
    attrs: u32,
}

impl Default for CellData {
    fn default() -> Self {
        Self {
            kind: 0,
            char_code: 0,
            grapheme: Vec::new(),
            fg: ftui_render::cell::PackedRgba::WHITE.0,
            bg: ftui_render::cell::PackedRgba::TRANSPARENT.0,
            attrs: 0,
        }
    }
}

fn fnv1a_update(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}

fn checksum_grid(cells: &[CellData]) -> u64 {
    let mut hash = FNV_OFFSET_BASIS;
    for cell in cells {
        fnv1a_update(&mut hash, &[cell.kind]);
        match cell.kind {
            0 | 3 => fnv1a_update(&mut hash, &0u16.to_le_bytes()),
            1 => {
                let ch = char::from_u32(cell.char_code).unwrap_or('\u{FFFD}');
                let mut buf = [0u8; 4];
                let encoded = ch.encode_utf8(&mut buf);
                let bytes = encoded.as_bytes();
                let len = u16::try_from(bytes.len()).unwrap_or(u16::MAX);
                fnv1a_update(&mut hash, &len.to_le_bytes());
                fnv1a_update(&mut hash, bytes);
            }
            2 => {
                let len = u16::try_from(cell.grapheme.len()).unwrap_or(u16::MAX);
                fnv1a_update(&mut hash, &len.to_le_bytes());
                fnv1a_update(&mut hash, &cell.grapheme[..len as usize]);
            }
            _ => fnv1a_update(&mut hash, &0u16.to_le_bytes()),
        }
        fnv1a_update(&mut hash, &cell.fg.to_le_bytes());
        fnv1a_update(&mut hash, &cell.bg.to_le_bytes());
        fnv1a_update(&mut hash, &cell.attrs.to_le_bytes());
    }
    hash
}

fn write_diff_runs(path: &Path, width: u16, height: u16, runs: &[Run]) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    file.write_all(&width.to_le_bytes())?;
    file.write_all(&height.to_le_bytes())?;
    let run_count = runs.len() as u32;
    file.write_all(&run_count.to_le_bytes())?;
    for run in runs {
        file.write_all(&run.y.to_le_bytes())?;
        file.write_all(&run.x0.to_le_bytes())?;
        file.write_all(&run.x1.to_le_bytes())?;
        for cell in &run.cells {
            file.write_all(&[cell.kind])?;
            match cell.kind {
                0 | 3 => {}
                1 => file.write_all(&cell.char_code.to_le_bytes())?,
                2 => {
                    let len = u16::try_from(cell.grapheme.len()).unwrap_or(u16::MAX);
                    file.write_all(&len.to_le_bytes())?;
                    file.write_all(&cell.grapheme)?;
                }
                _ => {}
            }
            file.write_all(&cell.fg.to_le_bytes())?;
            file.write_all(&cell.bg.to_le_bytes())?;
            file.write_all(&cell.attrs.to_le_bytes())?;
        }
    }
    Ok(())
}

struct Run {
    y: u16,
    x0: u16,
    x1: u16,
    cells: Vec<CellData>,
}

fn unique_temp_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("ftui_trace_replay_{nanos}"))
}

fn logger() -> &'static TestJsonlLogger {
    static LOGGER: OnceLock<TestJsonlLogger> = OnceLock::new();
    LOGGER.get_or_init(|| {
        let mut logger = TestJsonlLogger::new("trace_replay", 1337);
        logger.add_context_str("suite", "trace_replay");
        logger
    })
}

#[test]
fn replay_trace_success_and_mismatch() {
    let base_dir = unique_temp_dir();
    let frames_dir = base_dir.join("frames");
    fs::create_dir_all(&frames_dir).expect("create temp dirs");

    logger().log_env();

    let width = 2u16;
    let height = 2u16;

    let mut grid = vec![CellData::default(); (width * height) as usize];
    let empty_checksum = checksum_grid(&grid);
    assert_eq!(
        empty_checksum, EXPECTED_CHECKSUM_EMPTY_2X2,
        "unexpected checksum for empty 2x2 grid"
    );

    let cell_a = CellData {
        kind: 1,
        char_code: 'A' as u32,
        ..Default::default()
    };
    grid[0] = cell_a.clone();

    let checksum0 = checksum_grid(&grid);
    assert_eq!(
        checksum0, EXPECTED_CHECKSUM_A_ONLY_2X2,
        "checksum stability regression for frame 0"
    );

    let cell_b = CellData {
        kind: 1,
        char_code: 'B' as u32,
        ..Default::default()
    };
    grid[1] = cell_b.clone();

    let checksum1 = checksum_grid(&grid);
    assert_eq!(
        checksum1, EXPECTED_CHECKSUM_AB_2X2,
        "checksum stability regression for frame 1"
    );

    logger().log(
        "trace_replay_frame",
        &[
            ("frame_idx", JsonValue::u64(0)),
            ("cols", JsonValue::u64(width as u64)),
            ("rows", JsonValue::u64(height as u64)),
            ("checksum", JsonValue::str(format!("{checksum0:016x}"))),
        ],
    );
    logger().log(
        "trace_replay_frame",
        &[
            ("frame_idx", JsonValue::u64(1)),
            ("cols", JsonValue::u64(width as u64)),
            ("rows", JsonValue::u64(height as u64)),
            ("checksum", JsonValue::str(format!("{checksum1:016x}"))),
        ],
    );

    let run0 = Run {
        y: 0,
        x0: 0,
        x1: 0,
        cells: vec![cell_a],
    };
    let run1 = Run {
        y: 0,
        x0: 1,
        x1: 1,
        cells: vec![cell_b],
    };

    let payload0 = frames_dir.join("frame_0000.bin");
    let payload1 = frames_dir.join("frame_0001.bin");
    write_diff_runs(&payload0, width, height, &[run0]).expect("write payload 0");
    write_diff_runs(&payload1, width, height, &[run1]).expect("write payload 1");

    let trace_path = base_dir.join("trace.jsonl");
    let mut trace = fs::File::create(&trace_path).expect("create trace");
    writeln!(
        trace,
        r#"{{"event":"trace_header","schema_version":"render-trace-v1","run_id":"test","seed":0}}"#
    )
    .unwrap();
    writeln!(
        trace,
        r#"{{"event":"frame","frame_idx":0,"cols":2,"rows":2,"payload_kind":"diff_runs_v1","payload_path":"frames/frame_0000.bin","checksum":"{:016x}"}}"#,
        checksum0
    )
    .unwrap();
    writeln!(
        trace,
        r#"{{"event":"frame","frame_idx":1,"cols":2,"rows":2,"payload_kind":"diff_runs_v1","payload_path":"frames/frame_0001.bin","checksum":"{:016x}"}}"#,
        checksum1
    )
    .unwrap();
    writeln!(
        trace,
        r#"{{"event":"trace_summary","total_frames":2,"final_checksum_chain":"{:016x}","elapsed_ms":1}}"#,
        checksum1
    )
    .unwrap();

    let summary = replay_trace(&trace_path).expect("replay should succeed");
    assert_eq!(summary.frames, 2);
    assert_eq!(summary.last_checksum, Some(EXPECTED_CHECKSUM_AB_2X2));

    let summary_repeat = replay_trace(&trace_path).expect("replay should be deterministic");
    assert_eq!(summary_repeat.frames, summary.frames);
    assert_eq!(summary_repeat.last_checksum, summary.last_checksum);

    let bad_trace = base_dir.join("trace_bad.jsonl");
    let mut trace_bad = fs::File::create(&bad_trace).expect("create bad trace");
    writeln!(
        trace_bad,
        r#"{{"event":"frame","frame_idx":0,"cols":2,"rows":2,"payload_kind":"diff_runs_v1","payload_path":"frames/frame_0000.bin","checksum":"{:016x}"}}"#,
        checksum0
    )
    .unwrap();
    writeln!(
        trace_bad,
        r#"{{"event":"frame","frame_idx":1,"cols":2,"rows":2,"payload_kind":"diff_runs_v1","payload_path":"frames/frame_0001.bin","checksum":"{:016x}"}}"#,
        checksum1 ^ 1
    )
    .unwrap();

    let err = replay_trace(&bad_trace).expect_err("replay should fail");
    assert!(
        err.to_string().contains("checksum mismatch"),
        "unexpected error: {err}"
    );
}
