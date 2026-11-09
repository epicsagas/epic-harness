use super::*;

#[tokio::test]
async fn issue_259_scoped_text_and_boolean_metrics_round_trip() {
    let pool = super::super::pool::test_memory_pool().await;
    super::super::schema::init_schema_pool(&pool).await.unwrap();
    let metrics = Metrics {
        total_sessions: 9,
        avg_success_rate: 0.8,
        last_session: Some("2026-11-09T01:02:03Z".into()),
        best_session: "2026-11-08T01:02:03Z".into(),
        trend: "improving".into(),
        epoch_class: Some(crate::shared::evolution::EpochClass::Improving),
        last_error_context: Some("synthetic diagnostic context".into()),
        reward_hacking_suspected: true,
        ..Metrics::default()
    };
    save_metrics_pool(&pool, &metrics, "alpha").await.unwrap();
    save_metrics_pool(&pool, &Metrics::default(), "beta")
        .await
        .unwrap();
    let actual = load_metrics_scoped_pool(&pool, Some("alpha"))
        .await
        .unwrap();
    assert_eq!(actual.last_session, metrics.last_session);
    assert_eq!(actual.best_session, metrics.best_session);
    assert_eq!(actual.trend, metrics.trend);
    assert_eq!(actual.epoch_class, metrics.epoch_class);
    assert_eq!(actual.last_error_context, metrics.last_error_context);
    assert!(actual.reward_hacking_suspected);
    assert_eq!(actual.total_sessions, 9);
    assert!(
        !load_metrics_scoped_pool(&pool, Some("beta"))
            .await
            .unwrap()
            .reward_hacking_suspected
    );
    assert!(
        load_metrics_scoped_pool(&pool, None)
            .await
            .unwrap()
            .reward_hacking_suspected
    );
}
