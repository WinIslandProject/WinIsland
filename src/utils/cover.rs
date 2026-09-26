use winisland_render::Image;

pub(crate) fn decode_cover_image(data: &[u8]) -> Option<Image> {
    Image::decode(data)
}
