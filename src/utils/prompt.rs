use std::io::{ self, Write };

pub fn prompt_confirm(prompt: &str, default: Option<bool>) -> io::Result<bool> {
    let mut input = String::new();

    loop {
        input.clear();

        match default {
            Some(true) => print!("{} (Y/n): ", prompt),
            Some(false) | None => print!("{} (y/N): ", prompt),
        }
        io::stdout().flush()?; // Make sure the prompt is immediately displayed

        io::stdin().read_line(&mut input)?;

        match input.trim().to_uppercase().as_str() {
            "Y" => {
                return Ok(true);
            }
            "N" => {
                return Ok(false);
            }
            "" =>
                match default {
                    Some(default) => {
                        return Ok(default);
                    }
                    None => {
                        continue;
                    }
                }
            _ => {
                continue;
            }
        }
    }
}
