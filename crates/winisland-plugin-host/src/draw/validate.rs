use winisland_plugin_api::draw::v2 as wire;

#[derive(Clone, Copy, Debug)]
pub struct ValidationError {
    pub offset: usize,
    pub reason: &'static str,
}

pub struct ValidatedCommand<'a> {
    pub opcode: u16,
    pub payload: &'a [u8],
}

pub struct ValidatedDrawList<'a> {
    pub logical_width: f32,
    pub logical_height: f32,
    pub commands: Vec<ValidatedCommand<'a>>,
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn take(&mut self, len: usize) -> Result<&'a [u8], &'static str> {
        let end = self.offset.checked_add(len).ok_or("length overflow")?;
        let bytes = self
            .bytes
            .get(self.offset..end)
            .ok_or("truncated payload")?;
        self.offset = end;
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, &'static str> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, &'static str> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().map_err(|_| "truncated u16")?,
        ))
    }

    fn u32(&mut self) -> Result<u32, &'static str> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().map_err(|_| "truncated u32")?,
        ))
    }

    fn u64(&mut self) -> Result<u64, &'static str> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().map_err(|_| "truncated u64")?,
        ))
    }

    fn finite(&mut self) -> Result<f32, &'static str> {
        let value = f32::from_bits(self.u32()?);
        if !value.is_finite() {
            return Err("non-finite number");
        }
        Ok(value)
    }

    fn coordinate(&mut self) -> Result<f32, &'static str> {
        let value = self.finite()?;
        if value.abs() > 1_000_000.0 {
            return Err("coordinate exceeds limit");
        }
        Ok(value)
    }

    fn angle(&mut self) -> Result<f32, &'static str> {
        let value = self.finite()?;
        if value.abs() > 1_000_000.0 {
            return Err("angle exceeds limit");
        }
        Ok(value)
    }

    fn nonnegative(&mut self) -> Result<f32, &'static str> {
        let value = self.coordinate()?;
        if value < 0.0 {
            return Err("negative dimension or radius");
        }
        Ok(value)
    }

    fn stroke_width(&mut self) -> Result<f32, &'static str> {
        let width = self.finite()?;
        if width <= 0.0 || width > 10_000.0 {
            return Err("stroke width exceeds limit");
        }
        Ok(width)
    }

    fn rect(&mut self) -> Result<(), &'static str> {
        self.coordinate()?;
        self.coordinate()?;
        self.nonnegative()?;
        self.nonnegative()?;
        Ok(())
    }

    fn text(&mut self, max: usize) -> Result<usize, &'static str> {
        let len = self.u32()? as usize;
        if len > max {
            return Err("text exceeds limit");
        }
        std::str::from_utf8(self.take(len)?).map_err(|_| "invalid UTF-8")?;
        Ok(len)
    }

    fn family(&mut self) -> Result<(), &'static str> {
        let len = self.u16()? as usize;
        if len > wire::MAX_FAMILY_BYTES {
            return Err("font family exceeds limit");
        }
        std::str::from_utf8(self.take(len)?).map_err(|_| "invalid font family UTF-8")?;
        Ok(())
    }

    fn text_flags(&mut self) -> Result<(), &'static str> {
        if self.u8()? > 1 || self.u8()? > 2 || self.u8()? > 1 || self.u8()? > 1 {
            return Err("invalid text style flags");
        }
        Ok(())
    }

    fn weight(&mut self) -> Result<(), &'static str> {
        if !(100..=900).contains(&self.u16()?) {
            return Err("font weight exceeds limit");
        }
        Ok(())
    }

    fn size(&mut self) -> Result<(), &'static str> {
        let size = self.finite()?;
        if size <= 0.0 || size > 10_000.0 {
            return Err("text size exceeds limit");
        }
        Ok(())
    }

    fn fit(&mut self) -> Result<(), &'static str> {
        if self.u8()? > 2 {
            return Err("invalid image fit");
        }
        Ok(())
    }

    fn finish(self) -> Result<(), &'static str> {
        if self.offset != self.bytes.len() {
            return Err("payload has trailing bytes");
        }
        Ok(())
    }
}

fn convex(points: &[(f32, f32)]) -> bool {
    let mut sign = 0_i8;
    let mut fan_absolute = 0.0_f64;
    let mut fan_signed = 0.0_f64;
    for index in 0..points.len() {
        let a = points[index];
        let b = points[(index + 1) % points.len()];
        let c = points[(index + 2) % points.len()];
        if a == b {
            return false;
        }
        let cross = (b.0 as f64 - a.0 as f64) * (c.1 as f64 - b.1 as f64)
            - (b.1 as f64 - a.1 as f64) * (c.0 as f64 - b.0 as f64);
        if cross != 0.0 {
            let next = if cross > 0.0 { 1 } else { -1 };
            if sign != 0 && sign != next {
                return false;
            }
            sign = next;
        }
    }
    for index in 1..points.len() - 1 {
        let a = points[0];
        let b = points[index];
        let c = points[index + 1];
        let cross = (b.0 as f64 - a.0 as f64) * (c.1 as f64 - a.1 as f64)
            - (b.1 as f64 - a.1 as f64) * (c.0 as f64 - a.0 as f64);
        fan_absolute += cross.abs();
        fan_signed += cross;
    }
    sign != 0 && (fan_absolute - fan_signed.abs()) <= fan_absolute * 1e-9
}

fn concat_affine(current: [f64; 6], next: [f64; 6]) -> [f64; 6] {
    let [a, b, c, d, e, f] = current;
    let [g, h, i, j, k, l] = next;
    [
        a * g + c * h,
        b * g + d * h,
        a * i + c * j,
        b * i + d * j,
        a * k + c * l + e,
        b * k + d * l + f,
    ]
}

fn validate_payload(
    opcode: u16,
    payload: &[u8],
    stacks: &mut [u8; 3],
    transforms: &mut Vec<[f64; 6]>,
    image_owned: &impl Fn(u64) -> bool,
) -> Result<(), &'static str> {
    let mut cursor = Cursor::new(payload);
    match opcode {
        wire::PUSH_CLIP_RECT | wire::PUSH_CLIP_ROUND_RECT => {
            cursor.rect()?;
            if opcode == wire::PUSH_CLIP_ROUND_RECT {
                cursor.nonnegative()?;
            }
            stacks[0] = stacks[0]
                .checked_add(1)
                .filter(|depth| *depth <= 64)
                .ok_or("clip stack overflow")?;
        }
        wire::POP_CLIP => {
            stacks[0] = stacks[0].checked_sub(1).ok_or("clip stack underflow")?;
        }
        wire::PUSH_TRANSFORM => {
            let mut matrix = [0.0; 6];
            for component in &mut matrix {
                *component = f64::from(cursor.coordinate()?);
            }
            let current = transforms
                .last()
                .copied()
                .unwrap_or([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
            let combined = concat_affine(current, matrix);
            if combined
                .iter()
                .any(|value| !value.is_finite() || value.abs() > 1_000_000_000.0)
            {
                return Err("cumulative transform exceeds limit");
            }
            transforms.push(combined);
            stacks[1] = stacks[1]
                .checked_add(1)
                .filter(|depth| *depth <= 64)
                .ok_or("transform stack overflow")?;
        }
        wire::POP_TRANSFORM => {
            stacks[1] = stacks[1]
                .checked_sub(1)
                .ok_or("transform stack underflow")?;
            transforms.pop();
        }
        wire::PUSH_ALPHA => {
            cursor.u8()?;
            stacks[2] = stacks[2]
                .checked_add(1)
                .filter(|depth| *depth <= 64)
                .ok_or("alpha stack overflow")?;
        }
        wire::POP_ALPHA => {
            stacks[2] = stacks[2].checked_sub(1).ok_or("alpha stack underflow")?;
        }
        wire::FILL_RECT | wire::FILL_ROUND_RECT => {
            cursor.rect()?;
            if opcode == wire::FILL_ROUND_RECT {
                cursor.nonnegative()?;
            }
            cursor.u32()?;
        }
        wire::FILL_CIRCLE => {
            cursor.coordinate()?;
            cursor.coordinate()?;
            cursor.nonnegative()?;
            cursor.u32()?;
        }
        wire::FILL_CONVEX_POLYGON => {
            let count = cursor.u16()? as usize;
            if !(3..=1024).contains(&count) {
                return Err("polygon point count exceeds limit");
            }
            let mut points = Vec::with_capacity(count);
            for _ in 0..count {
                points.push((cursor.coordinate()?, cursor.coordinate()?));
            }
            if !convex(&points) {
                return Err("polygon is not convex");
            }
            cursor.u32()?;
        }
        wire::FILL_LINEAR_GRADIENT => {
            cursor.rect()?;
            cursor.angle()?;
            let stops = cursor.u8()?;
            if !(2..=16).contains(&stops) {
                return Err("gradient stop count exceeds limit");
            }
            let mut previous = 0.0;
            for _ in 0..stops {
                let position = cursor.finite()?;
                if !(0.0..=1.0).contains(&position) || position < previous {
                    return Err("gradient stops are not ordered");
                }
                previous = position;
                cursor.u32()?;
            }
        }
        wire::STROKE_LINE => {
            for _ in 0..4 {
                cursor.coordinate()?;
            }
            cursor.stroke_width()?;
            cursor.u32()?;
        }
        wire::STROKE_ROUND_RECT => {
            cursor.rect()?;
            cursor.nonnegative()?;
            cursor.stroke_width()?;
            cursor.u32()?;
        }
        wire::STROKE_ARC => {
            cursor.rect()?;
            cursor.angle()?;
            cursor.angle()?;
            cursor.stroke_width()?;
            cursor.u32()?;
        }
        wire::DRAW_IMAGE => {
            let id = cursor.u64()?;
            if id == 0 || !image_owned(id) {
                return Err("image handle is stale or belongs to another plugin");
            }
            cursor.rect()?;
            cursor.fit()?;
            cursor.u8()?;
        }
        wire::DRAW_IMAGE_PIXELS => {
            cursor.rect()?;
            cursor.fit()?;
            cursor.u8()?;
            let width = cursor.u32()?;
            let height = cursor.u32()?;
            let len = cursor.u32()? as usize;
            if width == 0
                || height == 0
                || width > 4096
                || height > 4096
                || u64::from(width) * u64::from(height) * 4 != len as u64
            {
                return Err("invalid inline image dimensions or length");
            }
            cursor.take(len)?;
        }
        wire::DRAW_TEXT => {
            cursor.rect()?;
            cursor.text(wire::MAX_TEXT_BYTES)?;
            cursor.size()?;
            cursor.weight()?;
            cursor.text_flags()?;
            cursor.family()?;
            cursor.u32()?;
        }
        wire::DRAW_TEXT_RUNS => {
            cursor.rect()?;
            cursor.size()?;
            cursor.text_flags()?;
            cursor.family()?;
            let count = cursor.u8()?;
            if !(1..=64).contains(&count) {
                return Err("text run count exceeds limit");
            }
            let mut total = 0_usize;
            for _ in 0..count {
                total = total
                    .checked_add(cursor.text(wire::MAX_TEXT_BYTES)?)
                    .ok_or("text length overflow")?;
                if total > wire::MAX_TEXT_BYTES {
                    return Err("text runs exceed limit");
                }
                cursor.u32()?;
                cursor.weight()?;
            }
        }
        wire::DRAW_SHADOW_ROUND_RECT => {
            cursor.rect()?;
            cursor.nonnegative()?;
            let sigma = cursor.finite()?;
            if !(0.0..=64.0).contains(&sigma) {
                return Err("shadow sigma exceeds limit");
            }
            cursor.coordinate()?;
            cursor.coordinate()?;
            cursor.u32()?;
        }
        _ => return Err("unknown opcode"),
    }
    cursor.finish()
}

pub fn validate<'a>(
    bytes: &'a [u8],
    image_owned: impl Fn(u64) -> bool,
) -> Result<ValidatedDrawList<'a>, ValidationError> {
    if bytes.len() > wire::MAX_LIST_BYTES {
        return Err(ValidationError {
            offset: 0,
            reason: "draw list exceeds limit",
        });
    }
    let mut cursor = Cursor::new(bytes);
    let header = (|| -> Result<(f32, f32, u32, u32), &'static str> {
        if cursor.u32()? != wire::MAGIC {
            return Err("invalid draw list magic");
        }
        if cursor.u32()? != wire::VERSION {
            return Err("unsupported draw list version");
        }
        let width = cursor.nonnegative()?;
        let height = cursor.nonnegative()?;
        let count = cursor.u32()?;
        let payload_len = cursor.u32()?;
        if count > wire::MAX_COMMANDS {
            return Err("too many draw commands");
        }
        if payload_len as usize != bytes.len().saturating_sub(24) {
            return Err("draw list length mismatch");
        }
        Ok((width, height, count, payload_len))
    })()
    .map_err(|reason| ValidationError {
        offset: cursor.offset,
        reason,
    })?;
    let mut commands = Vec::with_capacity(header.2 as usize);
    let mut stacks = [0_u8; 3];
    let mut transforms = Vec::new();
    for _ in 0..header.2 {
        let offset = cursor.offset;
        let opcode = cursor
            .u16()
            .map_err(|reason| ValidationError { offset, reason })?;
        let flags = cursor
            .u16()
            .map_err(|reason| ValidationError { offset, reason })?;
        let len = cursor
            .u32()
            .map_err(|reason| ValidationError { offset, reason })? as usize;
        if flags != 0 || len > wire::MAX_PAYLOAD_BYTES {
            return Err(ValidationError {
                offset,
                reason: "invalid command flags or payload size",
            });
        }
        let payload = cursor
            .take(len)
            .map_err(|reason| ValidationError { offset, reason })?;
        validate_payload(opcode, payload, &mut stacks, &mut transforms, &image_owned)
            .map_err(|reason| ValidationError { offset, reason })?;
        commands.push(ValidatedCommand { opcode, payload });
    }
    if cursor.offset != bytes.len() || stacks != [0; 3] {
        return Err(ValidationError {
            offset: cursor.offset,
            reason: "trailing bytes or unbalanced stack",
        });
    }
    Ok(ValidatedDrawList {
        logical_width: header.0,
        logical_height: header.1,
        commands,
    })
}
