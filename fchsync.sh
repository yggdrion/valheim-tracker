#!/usr/bin/env bash
# fchsync.sh: upload changed Valheim character files (.fch) to the tracker. Read-only on the game folder.
#   TRACKER_TOKEN=secret ./fchsync.sh [-d DIR] [-s http://host:8088] [-1]
# Needs curl and stat (GNU or BSD). Linux/macOS/Steam Deck.
set -u
SERVER=${TRACKER_SERVER:-http://localhost:8088}; TOKEN=${TRACKER_TOKEN:-}; DIR=${FCH_DIR:-}; ONCE=0
EVERY=2; HB=30
while getopts "d:s:t:1" o; do case $o in d) DIR=$OPTARG;; s) SERVER=$OPTARG;; t) TOKEN=$OPTARG;; 1) ONCE=1;; *) exit 2;; esac; done
[ -n "$TOKEN" ] || { echo "set TRACKER_TOKEN or pass -t" >&2; exit 2; }
if [ -z "$DIR" ]; then
  for c in "$HOME"/.config/unity3d/IronGate/Valheim/characters_local \
           "$HOME"/.local/share/Steam/userdata/*/892970/remote/characters \
           "$HOME"/.steam/steam/userdata/*/892970/remote/characters \
           "$HOME"/Library/Application\ Support/Steam/userdata/*/892970/remote/characters \
           "$HOME"/Library/Application\ Support/unity.IronGate.Valheim/characters_local \
           "$HOME"/.local/share/Steam/steamapps/compatdata/892970/pfx/drive_c/users/steamuser/AppData/LocalLow/IronGate/Valheim/characters_local; do
    [ -d "$c" ] && ls "$c"/*.fch >/dev/null 2>&1 && { DIR=$c; break; }
  done
fi
[ -d "$DIR" ] || { echo "characters folder not found, pass -d DIR" >&2; exit 2; }
echo "watching $DIR -> $SERVER" >&2
sig() { stat -c '%Y-%s' "$1" 2>/dev/null || stat -f '%m-%z' "$1"; }
urlenc() { printf %s "$1" | od -An -tx1 -v | tr -d '\n' | sed 's/ /%/g'; }
declare -A SIG BEAT
while :; do
  for f in "$DIR"/*.fch; do
    [ -e "$f" ] || continue
    name=$(basename "$f" .fch); case $name in *_backup_*) continue;; esac
    s=$(sig "$f"); now=$(date +%s); q=$(urlenc "$name")
    if [ "${SIG[$name]:-}" = "$s" ]; then
      if [ $((now - ${BEAT[$name]:-0})) -ge $HB ]; then
        code=$(curl -s -o /dev/null -w '%{http_code}' -m 10 -X POST -H "X-Token: $TOKEN" "$SERVER/heartbeat?name=$q")
        if [ "$code" = 404 ]; then unset 'SIG[$name]'; else BEAT[$name]=$now; fi
      fi
      continue
    fi
    sleep 0.7; [ "$(sig "$f")" = "$s" ] || continue   # game writes .fch.new then renames: wait until stable
    out=$(curl -s -m 15 -w '\n%{http_code}' -X POST -H "X-Token: $TOKEN" --data-binary @"$f" "$SERVER/upload?name=$q") || { echo "$name: server unreachable (retry)" >&2; continue; }
    code=${out##*$'\n'}
    if [ "$code" = 200 ]; then SIG[$name]=$s; BEAT[$name]=$now; echo "$name: uploaded" >&2
    elif [ "$code" = 400 ]; then SIG[$name]=$s; BEAT[$name]=$now; echo "$name: rejected: ${out%$'\n'*} (skipped until the file changes)" >&2
    else echo "$name: server said $code ${out%$'\n'*}" >&2; fi
  done
  [ "$ONCE" = 1 ] && exit 0
  sleep $EVERY
done
