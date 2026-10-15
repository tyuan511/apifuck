#!/usr/bin/env bash
# Executed on the host by GitHub Actions, inside /opt/apifuck-website.
set -euo pipefail
cd /opt/apifuck-website
: "${IMAGE_TAG:?IMAGE_TAG must be a commit SHA}"
[[ "$IMAGE_TAG" =~ ^[0-9a-f]{40}$ ]] || { echo 'Invalid commit SHA'; exit 1; }
previous=''
if [[ -f .env ]]; then previous=$(sed -n 's/^IMAGE_TAG=//p' .env); fi
rollback() {
  echo 'Deployment failed; restoring the previous image.'
  if [[ "$previous" =~ ^[0-9a-f]{40}$ ]]; then
    IMAGE_TAG="$previous" docker compose up -d --wait --wait-timeout 90 || true
  fi
}
trap rollback ERR
# Pull before replacing the running container. IMAGE_TAG is exported by the caller.
docker compose pull
docker compose up -d --wait --wait-timeout 90
curl --fail --silent --show-error http://127.0.0.1:9092/health.json | \
  python3 -c 'import json,os,sys; assert json.load(sys.stdin)["revision"] == os.environ["IMAGE_TAG"]'
curl --fail --silent --show-error http://127.0.0.1:9092/en/ -o /dev/null
# Persist the verified revision only after it serves successfully.
printf 'IMAGE_TAG=%s\n' "$IMAGE_TAG" > .env.next
mv .env.next .env
trap - ERR
docker compose ps
