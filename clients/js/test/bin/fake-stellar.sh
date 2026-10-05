#!/usr/bin/env bash
# Fake `stellar` CLI for tests: records argv, returns canned JSON.
set -euo pipefail
echo "$@" >>"${FAKE_STELLAR_LOG:?}"
FN=""
prev=""
for a in "$@"; do
  if [ "${prev}" = "--" ]; then FN="${a}"; break; fi
  prev="${a}"
done
case "${FN}" in
get_stats)
  echo '{"lockedCount":0,"lockedTotal":"0","releasedCount":0,"lifetimeVolume":"0"}'
  ;;
list_requests) echo '[]' ;;
*)
  echo '{"requestId":"00","depositor":"G","amount":"1","deadline":1,"status":"Locked","createdAt":1,"updatedAt":1}'
  ;;
esac
