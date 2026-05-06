<?php

$GLOBALS['_app_config'] ??= file_exists($f = __DIR__ . '/config.php') ? require $f : [];

function analytics_log(string $event, array $props = []): void {
    $db_path = $GLOBALS['_app_config']['db'] ?? '/tmp/logiquiz-analytics.sqlite3';

    $db = new SQLite3($db_path);
    $db->exec("CREATE TABLE IF NOT EXISTS events (
        id    INTEGER PRIMARY KEY AUTOINCREMENT,
        ts    TEXT DEFAULT (datetime('now')),
        event TEXT NOT NULL,
        props TEXT
    )");

    $stmt = $db->prepare("INSERT INTO events (event, props) VALUES (?, ?)");
    $stmt->bindValue(1, $event);
    $stmt->bindValue(2, $props ? json_encode($props,  JSON_UNESCAPED_UNICODE) : null);
    $stmt->execute();
}
