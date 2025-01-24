use anyhow::bail;
use base64::{prelude::BASE64_STANDARD, Engine};

pub fn get_iterm_image_encoding(url: &str) -> anyhow::Result<String> {
    let response = ureq::get(url).call()?;

    match response.header("content-length") {
        Some(content_length) => {
            let length: usize = content_length.parse()?;
            let mut file_bytes: Vec<u8> = vec![0; length];

            response.into_reader().read_exact(&mut file_bytes)?;

            let base64_emote = BASE64_STANDARD.encode(file_bytes);

            Ok(base64_emote)
        }

        // TODO: Add a hardcoded encoded placeholder image
        None => bail!("ERROR"),
    }
}
