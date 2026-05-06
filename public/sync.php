<?php

require_once __DIR__ . '/analytics-helper.php';

$dir = $GLOBALS['_app_config']['dir'] ?? '/tmp/refpuzzle-sync';
$max_age = 300; // 5 minutes
$max_files = 50;
$max_body = 102400; // 100 KB

if (!is_dir($dir)) mkdir($dir, 0700, true);

// Cleanup expired files
foreach (glob("$dir/*.json") as $f) {
    if (time() - filemtime($f) > $max_age) unlink($f);
}

header('Content-Type: application/json');

if ($_SERVER['REQUEST_METHOD'] === 'POST') {
    $body = file_get_contents('php://input');
    if (strlen($body) > $max_body || strlen($body) === 0) {
        http_response_code(400);
        echo json_encode(['error' => 'Invalid body']);
        exit;
    }

    $code = $_GET['code'] ?? null;
    $side = $_GET['side'] ?? null;

    if ($code && $side === 'b') {
        if (!preg_match('/^[0-9]{6}$/', $code)) {
            http_response_code(400);
            echo json_encode(['error' => 'Invalid code']);
            exit;
        }
        if (!file_exists("$dir/$code-a.json")) {
            http_response_code(404);
            echo json_encode(['error' => 'Code not found']);
            exit;
        }
        file_put_contents("$dir/$code-b.json", $body);
        analytics_log('sync_completed');
        echo json_encode(['ok' => true]);
        exit;
    }

    // New sync — check file cap
    $count = count(glob("$dir/*.json"));
    if ($count >= $max_files) {
        http_response_code(503);
        echo json_encode(['error' => 'Too busy, try again later']);
        exit;
    }

    // Generate unique 6-digit code
    do {
        $code = str_pad((string)random_int(0, 999999), 6, '0', STR_PAD_LEFT);
    } while (file_exists("$dir/$code-a.json"));

    file_put_contents("$dir/$code-a.json", $body);
    analytics_log('sync_started');
    echo json_encode(['code' => $code]);
    exit;
}

if ($_SERVER['REQUEST_METHOD'] === 'GET') {
    $code = $_GET['code'] ?? '';
    $side = $_GET['side'] ?? '';

    if (!preg_match('/^[0-9]{6}$/', $code) || !in_array($side, ['a', 'b'])) {
        http_response_code(400);
        echo json_encode(['error' => 'Invalid request']);
        exit;
    }

    $file = "$dir/$code-$side.json";
    if (!file_exists($file)) {
        http_response_code(404);
        echo json_encode(['error' => 'Not found']);
        exit;
    }

    echo file_get_contents($file);
    exit;
}

http_response_code(405);
echo json_encode(['error' => 'Method not allowed']);
