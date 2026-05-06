use codescout::retrieval::embedder::EmbedderHttp;

#[tokio::test]
async fn embedder_returns_dense_and_sparse() {
    let mut server = mockito::Server::new_async().await;
    let dense_mock = server.mock("POST", "/embed")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[[0.1, 0.2, 0.3]]"#)
        .create_async().await;
    let sparse_mock = server.mock("POST", "/embed_sparse")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"[[{"index":42,"value":0.5},{"index":7,"value":0.8}]]"#)
        .create_async().await;

    let eb = EmbedderHttp::new(server.url(), 3);
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
    let mut server = mockito::Server::new_async().await;
    server.mock("POST", "/embed")
        .with_status(200)
        .with_body(r#"[[0.1, 0.2]]"#)
        .create_async().await;
    server.mock("POST", "/embed_sparse")
        .with_status(200)
        .with_body(r#"[[]]"#)
        .create_async().await;

    let eb = EmbedderHttp::new(server.url(), 1024);
    let err = eb.embed("hi").await.unwrap_err();
    assert!(err.to_string().contains("dim"), "got: {err}");
}
