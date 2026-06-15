//! Keccak-f[1600] sponge and the SHA-3 / SHAKE functions ML-KEM is built on (Stage 14.1).
//!
//! Self-contained (R5/R39: no external crypto crate; the permutation is public-domain
//! NIST FIPS 202). Correctness is anchored to the published NIST digests in the tests
//! (e.g. `SHA3-256("") = a7ffc6f8…`), so this is KAT-validated, not merely self-consistent.
//!
//! Exposed: [`sha3_256`], [`sha3_512`], [`shake128`], [`shake256`]. ML-KEM's H, G, PRF,
//! XOF, J are thin wrappers over these (built in `crate::mlkem`).

const ROUNDS: usize = 24;

const RC: [u64; ROUNDS] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

const ROT: [[u32; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

/// The Keccak-f[1600] permutation on a 25-lane state.
fn keccak_f1600(s: &mut [u64; 25]) {
    for &rc in RC.iter() {
        // θ
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = s[x] ^ s[x + 5] ^ s[x + 10] ^ s[x + 15] ^ s[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for x in 0..5 {
            for y in 0..5 {
                s[x + 5 * y] ^= d[x];
            }
        }
        // ρ and π
        let mut b = [0u64; 25];
        for x in 0..5 {
            for y in 0..5 {
                let nx = y;
                let ny = (2 * x + 3 * y) % 5;
                b[nx + 5 * ny] = s[x + 5 * y].rotate_left(ROT[x][y]);
            }
        }
        // χ
        for x in 0..5 {
            for y in 0..5 {
                s[x + 5 * y] = b[x + 5 * y] ^ ((!b[(x + 1) % 5 + 5 * y]) & b[(x + 2) % 5 + 5 * y]);
            }
        }
        // ι
        s[0] ^= rc;
    }
}

/// Sponge: absorb `input` at the given `rate` (bytes) with domain-separation byte `pad`,
/// then squeeze `out_len` bytes. `rate = 200 - 2·security_bytes`.
fn keccak(rate: usize, pad: u8, input: &[u8], out_len: usize) -> Vec<u8> {
    debug_assert!(rate > 0 && rate < 200);
    let mut state = [0u64; 25];
    // absorb
    let mut blocks = input.chunks_exact(rate);
    for block in blocks.by_ref() {
        absorb_block(&mut state, block, rate);
        keccak_f1600(&mut state);
    }
    // pad the final (partial) block: pX byte at the start of padding, 0x80 at the end.
    let rem = blocks.remainder();
    let mut last = vec![0u8; rate];
    last[..rem.len()].copy_from_slice(rem);
    last[rem.len()] ^= pad;
    last[rate - 1] ^= 0x80;
    absorb_block(&mut state, &last, rate);
    keccak_f1600(&mut state);
    // squeeze
    let mut out = Vec::with_capacity(out_len);
    loop {
        for lane in state.iter().take(rate / 8) {
            if out.len() >= out_len {
                out.truncate(out_len);
                return out;
            }
            out.extend_from_slice(&lane.to_le_bytes());
        }
        if out.len() >= out_len {
            out.truncate(out_len);
            return out;
        }
        keccak_f1600(&mut state);
    }
}

fn absorb_block(state: &mut [u64; 25], block: &[u8], rate: usize) {
    debug_assert_eq!(block.len(), rate);
    for (i, lane) in state.iter_mut().enumerate().take(rate / 8) {
        let mut buf = [0u8; 8];
        buf.copy_from_slice(&block[8 * i..8 * i + 8]);
        *lane ^= u64::from_le_bytes(buf);
    }
}

/// SHA3-256 (32-byte digest).
pub fn sha3_256(input: &[u8]) -> [u8; 32] {
    let v = keccak(136, 0x06, input, 32);
    let mut out = [0u8; 32];
    out.copy_from_slice(&v);
    out
}

/// SHA3-512 (64-byte digest).
pub fn sha3_512(input: &[u8]) -> [u8; 64] {
    let v = keccak(72, 0x06, input, 64);
    let mut out = [0u8; 64];
    out.copy_from_slice(&v);
    out
}

/// SHAKE128 XOF → `out_len` bytes (rate 168).
pub fn shake128(input: &[u8], out_len: usize) -> Vec<u8> {
    keccak(168, 0x1f, input, out_len)
}

/// SHAKE256 XOF → `out_len` bytes (rate 136).
pub fn shake256(input: &[u8], out_len: usize) -> Vec<u8> {
    keccak(136, 0x1f, input, out_len)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(b: &[u8]) -> String {
        b.iter().map(|x| format!("{x:02x}")).collect()
    }

    #[test]
    fn sha3_256_known_answer() {
        // NIST FIPS 202 KATs.
        assert_eq!(
            hex(&sha3_256(b"")),
            "a7ffc6f8bf1ed76651c14756a061d662f580ff4de43b49fa82d80a4b80f8434a"
        );
        assert_eq!(
            hex(&sha3_256(b"abc")),
            "3a985da74fe225b2045c172d6bd390bd855f086e3e9d525b46bfe24511431532"
        );
    }

    #[test]
    fn sha3_512_known_answer() {
        assert_eq!(
            hex(&sha3_512(b"")),
            "a69f73cca23a9ac5c8b567dc185a756e97c982164fe25859e0d1dcc1475c80a6\
             15b2123af1f5f94c11e3e9402c3ac558f500199d95b6d3e301758586281dcd26"
        );
    }

    #[test]
    fn shake_known_answer() {
        // SHAKE128("") first 32 bytes (FIPS 202).
        assert_eq!(
            hex(&shake128(b"", 32)),
            "7f9c2ba4e88f827d616045507605853ed73b8093f6efbc88eb1a6eacfa66ef26"
        );
        // SHAKE256("") first 32 bytes.
        assert_eq!(
            hex(&shake256(b"", 32)),
            "46b9dd2b0ba88d13233b3feb743eeb243fcd52ea62b81b82b50c27646ed5762f"
        );
    }

    #[test]
    fn xof_is_a_prefix_stream() {
        // squeezing more bytes only extends the stream (sponge property).
        let a = shake128(b"jeff", 16);
        let b = shake128(b"jeff", 64);
        assert_eq!(a, b[..16]);
    }
}
