<?php

require_once __DIR__ . '/analytics-helper.php';

if ($_SERVER['REQUEST_METHOD'] !== 'POST') {
    http_response_code(405);
    exit;
}

$body = file_get_contents('php://input');
if (!$body) {
    http_response_code(400);
    exit;
}

$data = json_decode($body, true);
$event = $data['event'] ?? null;
if (!$event || !is_string($event)) {
    http_response_code(400);
    exit;
}

$props = isset($data['props']) && is_array($data['props']) ? $data['props'] : [];
analytics_log($event, $props);

http_response_code(204);
