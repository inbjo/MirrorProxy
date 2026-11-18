#!/usr/bin/env bash
set -Eeuo pipefail

# Full profile matrix: report missing authentic images as BLOCKED, never PASS.
root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
targets="${MIRRORPROXY_OS_SMOKE_TARGETS:-debian ubuntu kali trisquel linuxlite linuxmint deepin zorin armbian proxmox raspios rhel fedora centos centos-stream rocky alma oraclelinux openeuler opencloudos openkylin anolis opensuse archlinux manjaro endeavouros alpine gentoo nixos void openwrt}"
report="${MIRRORPROXY_OS_SMOKE_REPORT_DIR:-$(mktemp -d /tmp/mirrorproxy-os-matrix.XXXXXX)}"
mkdir -p "${report}"
failed=0
printf 'target\tstatus\texit_code\n' >"${report}/results.tsv"
for target in ${targets}; do
  [[ "${target}" =~ ^[a-z][a-z0-9-]*$ ]] || { echo "invalid matrix target: ${target}" >&2; exit 2; }
  status=0
  MIRRORPROXY_OS_SMOKE_TARGETS="${target}" \
    timeout --kill-after=15 "${MIRRORPROXY_OS_SMOKE_TIMEOUT:-1200}" \
    bash "${root}/scripts/smoke-os-clients.sh" >"${report}/${target}.log" 2>&1 || status=$?
  if ((status == 0)); then result=PASS
  elif ((status == 2)); then result=BLOCKED; failed=1
  else result=FAIL; failed=1
  fi
  printf '%s\t%s\t%s\n' "${target}" "${result}" "${status}" | tee -a "${report}/results.tsv"
done
printf 'Logs and results: %s\n' "${report}"
exit "${failed}"
