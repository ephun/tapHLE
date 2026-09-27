#!/bin/sh
set -eu
case "${1:-}" in
    --build|--install|--open|--version|--help) exec cmake "$@" ;;
    *) exec cmake -DCMAKE_POLICY_VERSION_MINIMUM=3.5 "$@" ;;
esac
