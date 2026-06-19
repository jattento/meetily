#!/usr/bin/env bash
set -euo pipefail

# Lanza Meetily en segundo plano, espera a que la API HTTP esté lista,
# y luego envía un archivo de audio a transcribir.
# Uso: ./scripts/codex/launch-and-transcribe.sh <ruta-al-audio> [título]

MEETILY_HTTP_PORT="${MEETILY_HTTP_PORT:-9517}"
MEETILY_URL="http://127.0.0.1:${MEETILY_HTTP_PORT}"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
MEETILY_BIN="${REPO_ROOT}/target/release/meetily"
AUDIO_FILE="${1:-}"
TITLE="${2:-}"

if [[ -z "$AUDIO_FILE" ]]; then
  echo "Uso: $0 <ruta-al-archivo-de-audio> [título]"
  exit 1
fi

if [[ ! -f "$AUDIO_FILE" ]]; then
  echo "Error: no existe el archivo '$AUDIO_FILE'"
  exit 1
fi

if [[ ! -x "$MEETILY_BIN" ]]; then
  echo "Error: no se encontró el binario de Meetily en $MEETILY_BIN"
  echo "Compilalo primero con: cargo build --release --manifest-path frontend/src-tauri/Cargo.toml"
  exit 1
fi

AUDIO_FILE_ABS="$(cd "$(dirname "$AUDIO_FILE")" && pwd)/$(basename "$AUDIO_FILE")"

# Launch Meetily in background, suppressing its UI/logs.
if ! curl -s "${MEETILY_URL}/health" >/dev/null 2>&1; then
  echo "Iniciando Meetily..."
  nohup "$MEETILY_BIN" >/tmp/meetily-nohup.log 2>&1 &
  MEETILY_PID=$!
  echo "Meetily PID: $MEETILY_PID"
else
  echo "Meetily ya está corriendo."
  MEETILY_PID=""
fi

# Wait for HTTP API to be ready (max 60s)
echo "Esperando que la API HTTP esté disponible en ${MEETILY_URL}..."
for i in $(seq 1 60); do
  if curl -s "${MEETILY_URL}/health" >/dev/null 2>&1; then
    echo "API lista."
    break
  fi
  sleep 1
done

if ! curl -s "${MEETILY_URL}/health" >/dev/null 2>&1; then
  echo "Error: la API HTTP no respondió después de 60 segundos."
  [[ -n "$MEETILY_PID" ]] && kill "$MEETILY_PID" 2>/dev/null || true
  exit 1
fi

# Invoke transcription
JSON_BODY='{"source_path":"'"$AUDIO_FILE_ABS"'"'
if [[ -n "$TITLE" ]]; then
  JSON_BODY+=',"title":"'"$TITLE"'"'
fi
JSON_BODY+='}'

echo "Enviando solicitud de transcripción para: $AUDIO_FILE_ABS"
RESPONSE=$(curl -s -X POST "${MEETILY_URL}/import-audio" \
  -H "Content-Type: application/json" \
  -d "$JSON_BODY")

echo "Respuesta: $RESPONSE"
echo "Podés seguir el progreso en la UI de Meetily o en /tmp/meetily-nohup.log"
