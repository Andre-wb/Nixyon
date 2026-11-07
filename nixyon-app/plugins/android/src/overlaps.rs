use std::path::Path;

use nixyon_core::ModelOverlaps;

use crate::SegmentsError;

/// Пишет перекрытия по модели сегментации в файл, который потом читает `read_overlaps`.
pub fn write_overlaps(path: &Path, overlaps: &ModelOverlaps) -> Result<(), SegmentsError> {
    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    serde_json::to_writer(file, overlaps)?;
    Ok(())
}

/// Читает перекрытия по модели сегментации. Перекрытие с концом раньше начала, нечисловое
/// время или окно без шага кадров значат битые данные.
pub fn read_overlaps(path: &Path) -> Result<ModelOverlaps, SegmentsError> {
    let file = std::io::BufReader::new(std::fs::File::open(path)?);
    let overlaps: ModelOverlaps = serde_json::from_reader(file)?;
    for (index, &(start, end)) in overlaps.spans.iter().enumerate() {
        if !start.is_finite() || !end.is_finite() || end < start {
            return Err(SegmentsError::BadOverlap { index });
        }
    }
    for (index, w) in overlaps.windows.iter().enumerate() {
        let frames_ok = w.frames.iter().flatten().all(|p| p.is_finite());
        let step_ok = w.frame_sec.is_finite() && w.frame_sec > 0.0;
        if !w.first_frame_sec.is_finite() || !step_ok || !frames_ok {
            return Err(SegmentsError::BadOverlapWindow { index });
        }
    }
    Ok(overlaps)
}

#[cfg(test)]
mod tests {
    use nixyon_core::PowersetWindow;

    use super::*;

    fn path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("nixyon-overlaps-{name}-{}", std::process::id()))
    }

    #[test]
    fn written_overlaps_are_read_back() {
        let overlaps = ModelOverlaps {
            spans: vec![(3.0, 4.0)],
            windows: vec![PowersetWindow {
                first_frame_sec: 0.03,
                frame_sec: 0.016875,
                frames: vec![[0.1, 0.2, 0.0, 0.0, 0.7, 0.0, 0.0]],
            }],
        };
        let file = path("ok");
        write_overlaps(&file, &overlaps).unwrap();
        assert_eq!(read_overlaps(&file).unwrap(), overlaps);
    }

    #[test]
    fn broken_overlaps_are_refused() {
        let back = path("back");
        std::fs::write(&back, r#"{"spans":[[2.0,1.0]],"windows":[]}"#).unwrap();
        assert!(matches!(
            read_overlaps(&back),
            Err(SegmentsError::BadOverlap { index: 0 })
        ));
        let step = path("step");
        std::fs::write(
            &step,
            r#"{"spans":[],"windows":[{"first_frame_sec":0.0,"frame_sec":0.0,"frames":[]}]}"#,
        )
        .unwrap();
        assert!(matches!(
            read_overlaps(&step),
            Err(SegmentsError::BadOverlapWindow { index: 0 })
        ));
        let cut = path("cut");
        std::fs::write(&cut, r#"{"spans":"#).unwrap();
        assert!(matches!(read_overlaps(&cut), Err(SegmentsError::Json(_))));
    }
}
