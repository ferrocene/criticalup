// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::Serialize;
use crate::{AuthenticationToken, Data};
use criticaltrust::manifests::ManifestVersion;
use tiny_http::{Header, Method, Request, Response, ResponseBox, StatusCode};

pub(crate) fn handle_request(data: &Data, req: &Request) -> ResponseBox {
    let url_parts = req
        .url()
        .split('/')
        .filter(|c| !c.is_empty())
        .collect::<Vec<_>>();

    let resp = match (req.method(), url_parts.as_slice()) {
        (Method::Get, ["v1", "tokens", "current"]) => handle_v1_tokens_current(data, req),
        (Method::Get, ["v1", "keys"]) => handle_v1_keys(data),
        (Method::Get, ["v1", "releases", product, release]) => {
            handle_v1_release(data, product, release)
        }
        _ => handle_404(),
    };

    // Handlers use `Result<Resp, Resp>` to be able to use `?` to propagate error responses. There
    // is no other difference between returning `Ok` or `Err`.
    match resp {
        Ok(resp) => resp.into_tiny_http(),
        Err(resp) => resp.into_tiny_http(),
    }
}

fn handle_v1_tokens_current(data: &Data, req: &Request) -> Result<Resp, Resp> {
    let token = authorize(data, req)?;
    Ok(Resp::json(token))
}

fn handle_v1_keys(data: &Data) -> Result<Resp, Resp> {
    Ok(Resp::json(&criticaltrust::manifests::KeysManifest {
        version: ManifestVersion,
        keys: data.keys.clone(),
        revoked_signatures: data.revoked_signatures.clone(),
    }))
}

fn handle_v1_release(data: &Data, product: &str, release: &str) -> Result<Resp, Resp> {
    let rm = data
        .release_manifests
        .get(&(product.to_string(), release.to_string()));
    let resp = Resp::json(rm.expect("Did not get a release manifest"));
    Ok(resp)
}

fn handle_404() -> Result<Resp, Resp> {
    Ok(Resp::NotFound)
}

fn authorize<'a>(data: &'a Data, req: &Request) -> Result<&'a AuthenticationToken, Resp> {
    let header = req
        .headers()
        .iter()
        .find(|h| h.field.equiv("authorization"))
        .ok_or(Resp::Forbidden)?;

    let without_prefix = header
        .value
        .as_str()
        .strip_prefix("Bearer ")
        .ok_or(Resp::Forbidden)?;

    if let Some(token) = data.tokens.get(without_prefix) {
        Ok(token)
    } else {
        Err(Resp::Forbidden)
    }
}

#[derive(Debug)]
enum Resp {
    Forbidden,
    NotFound,
    Json(Vec<u8>),
}

impl Resp {
    fn json<T: Serialize>(data: &T) -> Resp {
        let serialized = serde_json::to_vec_pretty(data).unwrap();
        Resp::Json(serialized)
    }

    fn into_tiny_http(self) -> ResponseBox {
        match self {
            Resp::Json(data) => Response::from_data(data)
                .with_status_code(StatusCode(200))
                .with_header(
                    Header::from_bytes(&b"Content-Type"[..], &b"application/json"[..]).unwrap(),
                )
                .boxed(),

            Resp::Forbidden => Response::empty(StatusCode(403)).boxed(),
            Resp::NotFound => Response::empty(StatusCode(404)).boxed(),
        }
    }
}
