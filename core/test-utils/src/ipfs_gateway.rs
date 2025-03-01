use axum::{http::HeaderMap, routing::get, Router};

pub async fn spawn_gateway(port: u16) -> anyhow::Result<()> {
    // Mostly taken from:
    // https://github.com/fleek-network/ursa/blob/main/crates/ursa-rpc-service/src/tests/mod.rs
    let file: Vec<u8> = std::fs::read(
        "../test-utils/files/bafybeigdyrzt5sfp7udm7hu76uh7y26nf3efuylqabf3oclgtqy55fbzdi",
    )?;

    let router = Router::new().route(
        "/ipfs/:cid",
        get(|| async move {
            let mut headers = HeaderMap::new();
            headers.insert("Content-Type", "application/vnd.ipld.raw".parse().unwrap());
            (headers, file.clone())
        }),
    );

    axum::Server::bind(&format!("0.0.0.0:{port}").parse().unwrap())
        .serve(router.into_make_service())
        .await
        .map_err(|e| e.into())
}
