pub trait ArrayLenExt {
    const LEN: usize;
}

impl<T, const N: usize> ArrayLenExt for [T; N] {
    const LEN: usize = N;
}
