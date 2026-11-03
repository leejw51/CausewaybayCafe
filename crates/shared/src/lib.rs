pub mod identity;
pub mod cafe_capnp {
    include!(concat!(env!("OUT_DIR"), "/cafe_capnp.rs"));
}
use serde::{Deserialize, Serialize};
use serde_json::Value;
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Rpc {
    pub method: String,
    pub token: String,
    pub payload: Value,
}
pub fn encode(r: &Rpc) -> Result<Vec<u8>, String> {
    let mut message = capnp::message::Builder::new_default();
    let data = serde_json::to_vec(&r.payload).map_err(|e| e.to_string())?;
    let mut root = message.init_root::<cafe_capnp::envelope::Builder>();
    root.set_version(1);
    root.set_method(r.method.as_str());
    root.set_token(r.token.as_str());
    root.set_payload(&data);
    let mut out = Vec::new();
    capnp::serialize::write_message(&mut out, &message).map_err(|e| e.to_string())?;
    Ok(out)
}
pub fn decode(bytes: &[u8]) -> Result<Rpc, String> {
    let mut options = capnp::message::ReaderOptions::new();
    options.traversal_limit_in_words(Some(1024 * 1024));
    options.nesting_limit(16);
    let message = capnp::serialize::read_message(bytes, options).map_err(|e| e.to_string())?;
    let r = message
        .get_root::<cafe_capnp::envelope::Reader>()
        .map_err(|e| e.to_string())?;
    if r.get_version() != 1 {
        return Err("Unsupported protocol version".into());
    }
    Ok(Rpc {
        method: r
            .get_method()
            .map_err(|e| e.to_string())?
            .to_str()
            .map_err(|e| e.to_string())?
            .into(),
        token: r
            .get_token()
            .map_err(|e| e.to_string())?
            .to_str()
            .map_err(|e| e.to_string())?
            .into(),
        payload: serde_json::from_slice(r.get_payload().map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_round_trip_and_reject_garbage() {
        let r = Rpc {
            method: "chat".into(),
            token: "abc".into(),
            payload: serde_json::json!({"text":"唔該一杯奶茶"}),
        };
        assert_eq!(decode(&encode(&r).unwrap()).unwrap().payload, r.payload);
        assert!(decode(b"invalid").is_err());
    }
}
