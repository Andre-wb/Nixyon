use crate::unit;

/// Синтетические «голоса»: группы единичных векторов вокруг случайных центров
/// размерности 64 с разбросом `spread`. Возвращает векторы и номер группы каждого.
/// Генератор детерминированный, чтобы тесты не плавали.
pub fn blobs(sizes: &[usize], spread: f32, seed: u64) -> (Vec<Vec<f32>>, Vec<usize>) {
    const DIM: usize = 64;
    let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut units = Vec::new();
    let mut labels = Vec::new();
    for (group, &size) in sizes.iter().enumerate() {
        let center: Vec<f32> = (0..DIM).map(|_| rng.normal()).collect();
        for _ in 0..size {
            let v: Vec<f32> = center
                .iter()
                .map(|c| c + spread * rng.normal() * 2.0)
                .collect();
            units.push(unit(&v).expect("ненулевой вектор"));
            labels.push(group);
        }
    }
    (units, labels)
}

/// Голоса, каждый записан в нескольких сессиях: у сессии свой сдвиг от центра
/// голоса (другой микрофон и комната). Номер группы у точки: номер голоса.
pub fn voices_with_sessions(
    voices: usize,
    sessions: usize,
    per_session: usize,
    session_shift: f32,
    spread: f32,
    seed: u64,
) -> (Vec<Vec<f32>>, Vec<usize>) {
    const DIM: usize = 64;
    let mut rng = Lcg(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1);
    let mut units = Vec::new();
    let mut labels = Vec::new();
    for voice in 0..voices {
        let center: Vec<f32> = (0..DIM).map(|_| rng.normal()).collect();
        for _ in 0..sessions {
            let session: Vec<f32> = center
                .iter()
                .map(|c| c + session_shift * rng.normal())
                .collect();
            for _ in 0..per_session {
                let v: Vec<f32> = session.iter().map(|c| c + spread * rng.normal()).collect();
                units.push(unit(&v).expect("ненулевой вектор"));
                labels.push(voice);
            }
        }
    }
    (units, labels)
}

struct Lcg(u64);

impl Lcg {
    fn next_f32(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 40) as f32 + 0.5) / (1u64 << 24) as f32
    }

    // Нормальное распределение, преобразование Бокса-Мюллера.
    fn normal(&mut self) -> f32 {
        let (u1, u2) = (self.next_f32(), self.next_f32());
        (-2.0 * u1.ln()).sqrt() * (2.0 * std::f32::consts::PI * u2).cos()
    }
}
