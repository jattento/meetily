#!/usr/bin/env bash
set -euo pipefail

# Script para transcribir un archivo de audio usando Meetily con API HTTP local.
# Uso: ./scripts/codex/transcribe-audio.sh /ruta/al/audio.mp3 ["Título opcional"]

MEETILY_HTTP_PORT="${MEETILY_HTTP_PORT:-9517}"
MEETILY_URL="http://127.0.0.1:${MEETILY_HTTP_PORT}"
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

AUDIO_FILE_ABS="$(cd "$(dirname "$AUDIO_FILE")" && pwd)/$(basename "$AUDIO_FILE")"

echo "Verificando que Meetily HTTP API esté disponible en ${MEETILY_URL}..."
if ! curl -s "${MEETILY_URL}/health" >/dev/null; then
  echo "Error: Meetily no responde en ${MEETILY_URL}. Asegurate de tener Meetily corriendo."
  exit 1
fi

echo "Enviando solicitud de transcripción para: $AUDIO_FILE_ABS"

JSON_BODY='{"source_path":"'"$AUDIO_FILE_ABS"'"'
if [[ -n "$TITLE" ]]; then
  JSON_BODY+=',"title":"'"$TITLE"'"'
fi
JSON_BODY+='}'

RESPONSE=$(curl -s -X POST "${MEETILY_URL}/import-audio" \
  -H "Content-Type: application/json" \
  -d "$JSON_BODY")

echo "Respuesta: $RESPONSE"
