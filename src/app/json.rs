//! The JSON form of a data item, so any command can offer `--json` for scripts and agents by
//! calling `write_json`. Every type that derives `Serialize` has it; a new data type needs
//! nothing more than that derive.

use std::io::Write;

use serde::Serialize;

pub trait Json {
    /// The item as pretty-printed JSON and a newline.
    fn write_json(&self, out: &mut dyn Write) -> anyhow::Result<()>;
}

impl<T: Serialize + ?Sized> Json for T {
    fn write_json(&self, out: &mut dyn Write) -> anyhow::Result<()> {
        serde_json::to_writer_pretty(&mut *out, self)?;
        writeln!(out)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    #[derive(Serialize)]
    struct Thing {
        name: &'static str,
    }

    fn written(item: &(impl Json + ?Sized)) -> Value {
        let mut out = Vec::new();
        item.write_json(&mut out).unwrap();
        assert_eq!(out.last(), Some(&b'\n'));
        serde_json::from_slice(&out).unwrap()
    }

    #[test]
    fn a_data_item_and_a_list_of_them_write_the_json_they_serialize_to() {
        let one = written(&Thing { name: "a" });
        let many = written(&vec![Thing { name: "a" }, Thing { name: "b" }]);

        assert_eq!(one, json!({"name": "a"}));
        assert_eq!(many, json!([{"name": "a"}, {"name": "b"}]));
    }
}
