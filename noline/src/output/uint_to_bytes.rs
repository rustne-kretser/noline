#[cfg_attr(test, derive(Debug))]
#[derive(Copy, Clone)]
pub struct UintToBytes<const N: usize> {
    bytes: [u8; N],
}

impl<const N: usize> UintToBytes<N> {
    pub(super) fn from_uint<I: Into<usize>>(n: I) -> Option<Self> {
        let mut n: usize = n.into();

        if n < 10_usize.pow(N as u32) {
            let mut bytes = [0; N];

            for i in (0..N).rev() {
                bytes[i] = 0x30 + (n % 10) as u8;
                n /= 10;

                if n == 0 {
                    break;
                }
            }

            Some(Self { bytes })
        } else {
            None
        }
    }

    pub fn as_bytes(&self) -> &[u8] {
        let start = self.bytes.iter().take_while(|&&b| b == 0).count();
        &self.bytes[start..]
    }
}
