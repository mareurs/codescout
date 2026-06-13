use codescout::retrieval::embedder::{DenseProtocol, EmbedderHttp};

#[tokio::test]

async fn embedder_returns_dense_and_sparse() {
    let mut dense_server = mockito::Server::new_async().await;
    let mut sparse_server = mockito::Server::new_async().await;
    let dense_mock = dense_server
        .mock("POST", "/embed")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[[0.1, 0.2, 0.3]]"#)
        .create_async()
        .await;
    let sparse_mock = sparse_server
        .mock("POST", "/embed_sparse")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[[{"index":42,"value":0.5},{"index":7,"value":0.8}]]"#)
        .create_async()
        .await;

    let eb = EmbedderHttp::with_protocol(
        dense_server.url(),
        sparse_server.url(),
        3,
        DenseProtocol::Tei,
        "",
        "",
    );
    let out = eb.embed("hello").await.expect("embed");

    assert_eq!(out.dense, vec![0.1_f32, 0.2, 0.3]);
    assert_eq!(out.sparse.indices, vec![42u32, 7]);
    assert!((out.sparse.values[0] - 0.5_f32).abs() < 1e-6);
    assert!((out.sparse.values[1] - 0.8_f32).abs() < 1e-6);
    dense_mock.assert_async().await;
    sparse_mock.assert_async().await;
}

#[tokio::test]

async fn embedder_dim_mismatch_errors() {
    let mut dense_server = mockito::Server::new_async().await;
    let mut sparse_server = mockito::Server::new_async().await;
    dense_server
        .mock("POST", "/embed")
        .with_status(200)
        .with_body(r#"[[0.1, 0.2]]"#)
        .create_async()
        .await;
    sparse_server
        .mock("POST", "/embed_sparse")
        .with_status(200)
        .with_body(r#"[[]]"#)
        .create_async()
        .await;

    let eb = EmbedderHttp::with_protocol(
        dense_server.url(),
        sparse_server.url(),
        1024,
        DenseProtocol::Tei,
        "",
        "",
    );
    let err = eb.embed("hi").await.unwrap_err();
    assert!(err.to_string().contains("dim"), "got: {err}");
}

use codescout::retrieval::reranker::{Protocol, RerankerHttp};

#[tokio::test]
async fn reranker_returns_scores_in_input_order() {
    let mut server = mockito::Server::new_async().await;
    server
        .mock("POST", "/rerank")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[{"index":1,"score":0.9},{"index":0,"score":0.1}]"#)
        .create_async()
        .await;

    let rr = RerankerHttp::with_protocol(server.url(), Protocol::Tei, None);
    let scores = rr
        .rerank("query", &["a".to_string(), "b".to_string()])
        .await
        .expect("rerank");
    assert_eq!(scores.len(), 2);
    assert!((scores[0] - 0.1_f32).abs() < 1e-6);
    assert!((scores[1] - 0.9_f32).abs() < 1e-6);
}

#[tokio::test]
async fn reranker_503_returns_error() {
    let mut server = mockito::Server::new_async().await;
    server
        .mock("POST", "/rerank")
        .with_status(503)
        .create_async()
        .await;
    let rr = RerankerHttp::with_protocol(server.url(), Protocol::Tei, None);
    let err = rr.rerank("q", &["a".to_string()]).await.unwrap_err();
    assert!(err.to_string().contains("rerank"), "got {err}");
}
