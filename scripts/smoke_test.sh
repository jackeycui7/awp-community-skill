#!/bin/sh
# Thin wrapper — delegates to `community-agent smoke-test`.
exec community-agent smoke-test "$@"
