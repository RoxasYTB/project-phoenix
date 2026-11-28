use super::{Depacketizer, Packetizer};
use crate::{leb128, Error, Result};

pub const OBU_SEQUENCE_HEADER: u8 = 1;
pub const OBU_TEMPORAL_DELIMITER: u8 = 2;
pub const OBU_TILE_LIST: u8 = 8;
pub const OBU_PADDING: u8 = 15;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Obu<'a> {
    pub header: Vec<u8>,
    pub payload: &'a [u8],
}

impl Obu<'_> {
    pub fn obu_type(&self) -> u8 {
        (self.header[0] >> 3) & 0x0f
    }
}

pub fn parse_obus(data: &[u8]) -> Result<Vec<Obu<'_>>> {
    let mut out = Vec::new();
    let mut o = 0;
    while o < data.len() {
        let h = data[o];
        let ext = h & 0x04 != 0;
        let has_size = h & 0x02 != 0;
        let hl = if ext { 2 } else { 1 };
        let hdr = data.get(o..o + hl).ok_or(Error::Malformed("obu header"))?;
        o += hl;
        let len = if has_size {
            let (l, n) = leb128::read(&data[o..])?;
            o += n;
            l as usize
        } else {
            data.len() - o
        };
        let payload = o.checked_add(len).and_then(|end| data.get(o..end)).ok_or(Error::Malformed("obu size"))?;
        o += len;
        let mut header = hdr.to_vec();
        header[0] &= !0x02;
        out.push(Obu { header, payload });
    }
    Ok(out)
}

pub fn strip_last_obu_size(frame: &[u8]) -> Result<Vec<u8>> {
    let obus: Vec<Obu<'_>> = parse_obus(frame)?
        .into_iter()
        .filter(|o| !matches!(o.obu_type(), OBU_TEMPORAL_DELIMITER | OBU_TILE_LIST | OBU_PADDING))
        .collect();
    let mut out = Vec::with_capacity(frame.len());
    let n = obus.len();
    for (i, o) in obus.iter().enumerate() {
        if i + 1 == n {
            out.extend_from_slice(&o.header);
        } else {
            out.push(o.header[0] | 0x02);
            out.extend_from_slice(&o.header[1..]);
            leb128::write(o.payload.len() as u64, &mut out);
        }
        out.extend_from_slice(o.payload);
    }
    Ok(out)
}

pub fn with_obu_sizes(frame: &[u8]) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(frame.len() + 8);
    for o in parse_obus(frame)? {
        out.push(o.header[0] | 0x02);
        out.extend_from_slice(&o.header[1..]);
        leb128::write(o.payload.len() as u64, &mut out);
        out.extend_from_slice(o.payload);
    }
    Ok(out)
}

pub fn without_last_obu_size(frame: &[u8]) -> Option<Vec<u8>> {
    let obus = parse_obus(frame).ok()?;
    let last = obus.len().checked_sub(1)?;
    let mut out = Vec::with_capacity(frame.len());
    for (i, o) in obus.iter().enumerate() {
        if i == last {
            out.extend_from_slice(&o.header);
        } else {
            out.push(o.header[0] | 0x02);
            out.extend_from_slice(&o.header[1..]);
            leb128::write(o.payload.len() as u64, &mut out);
        }
        out.extend_from_slice(o.payload);
    }
    Some(out)
}

pub fn is_keyframe(frame: &[u8]) -> bool {
    parse_obus(frame).is_ok_and(|obus| obus.iter().any(|o| o.obu_type() == OBU_SEQUENCE_HEADER))
}

#[derive(Default)]
pub struct Av1Packetizer;

impl Packetizer for Av1Packetizer {
    fn packetize(&mut self, frame: &[u8], max_payload: usize) -> Result<Vec<Vec<u8>>> {
        if max_payload < 8 {
            return Err(Error::Malformed("mtu too small"));
        }
        let obus = parse_obus(frame)?;
        let elements: Vec<Vec<u8>> = obus
            .iter()
            .filter(|o| !matches!(o.obu_type(), OBU_TEMPORAL_DELIMITER | OBU_TILE_LIST | OBU_PADDING))
            .map(|o| {
                let mut e = o.header.clone();
                e.extend_from_slice(o.payload);
                e
            })
            .collect();
        if elements.is_empty() {
            return Err(Error::Malformed("no OBUs to send"));
        }
        let new_sequence = obus.iter().any(|o| o.obu_type() == OBU_SEQUENCE_HEADER);

        let mut packets: Vec<Vec<u8>> = Vec::new();
        let mut cur = vec![0u8];
        let mut z = false;
        for el in &elements {
            let mut rest: &[u8] = el;
            loop {
                let space = max_payload.saturating_sub(cur.len());
                let need = leb128::encoded_len(rest.len() as u64) + rest.len();
                if need <= space {
                    leb128::write(rest.len() as u64, &mut cur);
                    cur.extend_from_slice(rest);
                    break;
                }
                if space >= 3 {
                    let mut take = space - leb128::encoded_len(space as u64);
                    take = take.min(rest.len());
                    leb128::write(take as u64, &mut cur);
                    cur.extend_from_slice(&rest[..take]);
                    rest = &rest[take..];
                    cur[0] = agg_header(z, true, packets.is_empty() && new_sequence);
                    packets.push(std::mem::replace(&mut cur, vec![0u8]));
                    z = true;
                } else {
                    cur[0] = agg_header(z, false, packets.is_empty() && new_sequence);
                    packets.push(std::mem::replace(&mut cur, vec![0u8]));
                    z = false;
                }
            }
        }
        if cur.len() > 1 {
            cur[0] = agg_header(z, false, packets.is_empty() && new_sequence);
            packets.push(cur);
        }
        Ok(packets)
    }
}

fn agg_header(z: bool, y: bool, n: bool) -> u8 {
    (z as u8) << 7 | (y as u8) << 6 | (n as u8) << 3
}

#[derive(Default)]
pub struct Av1Depacketizer {
    frame: Vec<u8>,
    partial: Vec<u8>,
}

impl Av1Depacketizer {
    fn emit_obu(&mut self, el: &[u8]) -> Result<()> {
        let h = *el.first().ok_or(Error::Malformed("empty obu element"))?;
        let hl = if h & 0x04 != 0 { 2 } else { 1 };
        if el.len() < hl {
            return Err(Error::Malformed("obu element header"));
        }
        self.frame.push(h | 0x02);
        self.frame.extend_from_slice(&el[1..hl]);
        leb128::write((el.len() - hl) as u64, &mut self.frame);
        self.frame.extend_from_slice(&el[hl..]);
        Ok(())
    }
}

impl Depacketizer for Av1Depacketizer {
    fn push(&mut self, p: &[u8], marker: bool) -> Result<Option<Vec<u8>>> {
        let agg = *p.first().ok_or(Error::Malformed("av1 empty"))?;
        let z = agg & 0x80 != 0;
        let y = agg & 0x40 != 0;
        let w = ((agg >> 4) & 0x03) as usize;
        let mut o = 1;
        let mut elements: Vec<&[u8]> = Vec::new();
        while o < p.len() {
            let last_implicit = w != 0 && elements.len() + 1 == w;
            let len = if last_implicit {
                p.len() - o
            } else {
                let (l, n) = leb128::read(&p[o..])?;
                o += n;
                l as usize
            };
            elements.push(o.checked_add(len).and_then(|end| p.get(o..end)).ok_or(Error::Malformed("av1 element"))?);
            o += len;
        }
        if !z && !self.partial.is_empty() {
            self.partial.clear();
        }
        let n = elements.len();
        for (i, el) in elements.into_iter().enumerate() {
            let continues = i + 1 == n && y;
            if i == 0 && z {
                self.partial.extend_from_slice(el);
                if !continues {
                    let whole = std::mem::take(&mut self.partial);
                    self.emit_obu(&whole)?;
                }
            } else if continues {
                self.partial = el.to_vec();
            } else {
                self.emit_obu(el)?;
            }
        }
        Ok(marker.then(|| std::mem::take(&mut self.frame)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn obu(t: u8, payload: &[u8]) -> Vec<u8> {
        let mut v = vec![(t << 3) | 0x02];
        leb128::write(payload.len() as u64, &mut v);
        v.extend_from_slice(payload);
        v
    }

    fn huge_leb128() -> Vec<u8> {
        let mut v = vec![0xff; 9];
        v.push(0x01);
        v
    }

    #[test]
    fn huge_obu_size_is_malformed() {
        let mut frame = vec![(6 << 3) | 0x02];
        frame.extend_from_slice(&huge_leb128());
        assert!(parse_obus(&frame).is_err());
    }

    #[test]
    fn huge_element_length_is_malformed() {
        let mut packet = vec![0x00];
        packet.extend_from_slice(&huge_leb128());
        assert!(Av1Depacketizer::default().push(&packet, true).is_err());
    }

    fn rt(frame: &[u8], mtu: usize) -> (Vec<Vec<u8>>, Vec<u8>) {
        let pl = Av1Packetizer.packetize(frame, mtu).unwrap();
        let mut d = Av1Depacketizer::default();
        let mut out = None;
        for (i, p) in pl.iter().enumerate() {
            assert!(p.len() <= mtu, "{} > {mtu}", p.len());
            out = d.push(p, i + 1 == pl.len()).unwrap();
        }
        (pl, out.unwrap())
    }

    #[test]
    fn roundtrip_fragmented_drops_td() {
        let seq = obu(1, &[0, 0, 0, 0x42]);
        let big: Vec<u8> = (0..4000u32).map(|i| (i * 7) as u8).collect();
        let fr = obu(6, &big);
        let mut input = obu(OBU_TEMPORAL_DELIMITER, &[]);
        input.extend_from_slice(&seq);
        input.extend_from_slice(&fr);
        for mtu in [100, 1100, 1200, 5000] {
            let (pl, out) = rt(&input, mtu);
            let mut expect = seq.clone();
            expect.extend_from_slice(&fr);
            assert_eq!(out, expect, "mtu {mtu}");
            assert!(pl[0][0] & 0x08 != 0, "N bit on new sequence");
            assert_eq!(pl.last().unwrap()[0] & 0x40, 0, "last Y clear");
        }
    }

    #[test]
    fn obu_size_fields_for_dave_and_the_decoder() {
        let seq = obu(OBU_SEQUENCE_HEADER, &[0, 0, 0, 0x42]);
        let fr = obu(6, &[9, 8, 7, 6, 5]);
        let mut sized = seq.clone();
        sized.extend_from_slice(&fr);
        assert!(is_keyframe(&sized));
        assert!(!is_keyframe(&fr));
        let bare = without_last_obu_size(&sized).unwrap();
        assert_eq!(bare.len(), sized.len() - 1);
        assert_eq!(bare[seq.len()], 6 << 3);
        assert_eq!(with_obu_sizes(&bare).unwrap(), sized);
        assert_eq!(without_last_obu_size(&[]), None);
    }

    #[test]
    fn small_frame_single_packet() {
        let input = obu(6, &[1, 2, 3]);
        let (pl, out) = rt(&input, 1100);
        assert_eq!(pl.len(), 1);
        assert_eq!(pl[0][0], 0);
        assert_eq!(out, input);
    }
}
