#!/usr/bin/env bash
set -euo pipefail

PROFILE="${CODESCOUT_RETRIEVAL_PROFILE:-cpu}"
COMPOSE="docker compose --profile ${PROFILE}"

case "${1:-help}" in
  up)    $COMPOSE up -d ;;
  down)  $COMPOSE down ;;
  logs)  $COMPOSE logs -f "${2:-}" ;;
  pull)  $COMPOSE pull ;;
  ps)    $COMPOSE ps ;;
  purge-legacy)
    find . -type d -name '.codescout' -prune -exec rm -rf {} +
    echo "Removed legacy .codescout/ directories"
    ;;
  help|*)
    cat <<EOF
Usage: $0 {up|down|logs|pull|ps|purge-legacy}
Profile: \$CODESCOUT_RETRIEVAL_PROFILE (default: cpu)
EOF
    ;;
esac
