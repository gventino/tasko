#!/usr/bin/env bash
#
# tasko installer
#
# Quick install (clones the repo, builds the release binary and installs it
# into ~/.cargo/bin):
#
#   curl -fsSL https://raw.githubusercontent.com/gventino/tasko/main/install.sh | bash
#
# Environment overrides:
#   TASKO_REPO    git URL to clone        (default: https://github.com/gventino/tasko)
#   TASKO_REF     branch / tag / commit   (default: remote HEAD)
#   TASKO_SKIP_RUST=1  do not auto-install Rust if cargo is missing
#
set -euo pipefail

REPO_URL="${TASKO_REPO:-https://github.com/gventino/tasko}"
REPO_REF="${TASKO_REF:-}"
MIN_RUST="1.88.0"   # repo uses let-chains + edition 2024
BIN_NAME="tasko"

# ---- pretty logging -------------------------------------------------------
if [ -t 1 ] && command -v tput >/dev/null 2>&1 && [ -n "$(tput colors 2>/dev/null || echo 0)" ]; then
  BOLD="$(tput bold)"; RED="$(tput setaf 1)"; GREEN="$(tput setaf 2)"
  YELLOW="$(tput setaf 3)"; BLUE="$(tput setaf 4)"; RESET="$(tput sgr0)"
else
  BOLD=""; RED=""; GREEN=""; YELLOW=""; BLUE=""; RESET=""
fi

info()  { printf '%s==>%s %s\n'  "$BLUE$BOLD" "$RESET" "$*"; }
ok()    { printf '%s ok %s %s\n' "$GREEN$BOLD" "$RESET" "$*"; }
warn()  { printf '%swarn%s %s\n' "$YELLOW$BOLD" "$RESET" "$*" >&2; }
die()   { printf '%serror%s %s\n' "$RED$BOLD" "$RESET" "$*" >&2; exit 1; }

# ---- cleanup --------------------------------------------------------------
TMP_DIR=""
cleanup() { [ -n "$TMP_DIR" ] && rm -rf "$TMP_DIR"; }
trap cleanup EXIT

# ---- helpers --------------------------------------------------------------
have() { command -v "$1" >/dev/null 2>&1; }

# version_ge A B  ->  true (0) when A >= B
version_ge() {
  [ "$1" = "$2" ] && return 0
  local IFS=. i a b
  local -a v1=($1) v2=($2)
  for ((i = 0; i < ${#v2[@]}; i++)); do
    a="${v1[i]:-0}"; b="${v2[i]:-0}"
    a="${a%%[!0-9]*}"; b="${b%%[!0-9]*}"   # strip pre-release suffixes
    a="${a:-0}"; b="${b:-0}"
    if ((10#$a > 10#$b)); then return 0; fi
    if ((10#$a < 10#$b)); then return 1; fi
  done
  return 0
}

rust_version() { rustc --version 2>/dev/null | awk '{print $2}'; }

# ---- ensure a C toolchain (libsqlite3-sys builds bundled SQLite) -----------
check_cc() {
  if ! have cc && ! have gcc && ! have clang; then
    warn "No C compiler found. Building SQLite needs one:"
    case "$(uname -s)" in
      Darwin)
        warn "  macOS:  xcode-select --install"
        ;;
      Linux)
        warn "  Debian/Ubuntu:  sudo apt-get install build-essential"
        warn "  Fedora:         sudo dnf groupinstall 'Development Tools'"
        ;;
    esac
  fi
}

# ---- ensure Rust toolchain ------------------------------------------------
ensure_rust() {
  if ! have cargo; then
    if [ "${TASKO_SKIP_RUST:-0}" = "1" ]; then
      die "cargo not found and TASKO_SKIP_RUST=1. Install Rust >= $MIN_RUST first: https://rustup.rs"
    fi
    info "Rust toolchain not found, installing via rustup..."
    have curl || die "curl is required to install Rust."
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    # shellcheck disable=SC1090
    [ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"
    export PATH="$HOME/.cargo/bin:$PATH"
    have cargo || die "Rust installation failed; cargo still not on PATH."
  fi

  local ver; ver="$(rust_version)"
  if [ -z "$ver" ] || ! version_ge "$ver" "$MIN_RUST"; then
    warn "Rust ${ver:-<unknown>} found, but >= $MIN_RUST is required."
    if have rustup; then
      info "Updating the stable toolchain..."
      rustup update stable
      rustup default stable >/dev/null 2>&1 || true
      ver="$(rust_version)"
    fi
    version_ge "${ver:-0}" "$MIN_RUST" \
      || die "Rust >= $MIN_RUST required (have ${ver:-none}). Update with: rustup update stable"
  fi
  ok "Using Rust $(rust_version)"
}

# ---- main -----------------------------------------------------------------
main() {
  printf '%s\n' "${BOLD}Installing ${BIN_NAME}${RESET}"

  have git || die "git is required."
  ensure_rust
  check_cc

  local src_dir
  if [ -f "Cargo.toml" ] && grep -q '^[[:space:]]*name[[:space:]]*=[[:space:]]*"tasko"' Cargo.toml 2>/dev/null; then
    src_dir="$(pwd)"
    info "Using current checkout: $src_dir"
  else
    TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t tasko)"
    info "Cloning $REPO_URL ..."
    if [ -n "$REPO_REF" ]; then
      git clone --depth 1 --branch "$REPO_REF" "$REPO_URL" "$TMP_DIR" 2>/dev/null \
        || { git clone "$REPO_URL" "$TMP_DIR" && git -C "$TMP_DIR" checkout --quiet "$REPO_REF"; }
    else
      git clone --depth 1 "$REPO_URL" "$TMP_DIR"
    fi
    src_dir="$TMP_DIR"
  fi

  info "Building and installing $BIN_NAME (release)..."
  cargo install --path "$src_dir" --locked --force

  local bin_path; bin_path="$(command -v "$BIN_NAME" 2>/dev/null || echo "$HOME/.cargo/bin/$BIN_NAME")"
  ok "Installed $BIN_NAME -> $bin_path"

  case ":$PATH:" in
    *":$HOME/.cargo/bin:"*) ;;
    *)
      warn "\$HOME/.cargo/bin is not on your PATH. Add this to your shell profile:"
      warn '  export PATH="$HOME/.cargo/bin:$PATH"'
      ;;
  esac

  printf '\n%sDone!%s Run %s%s%s to start.\n' \
    "$GREEN$BOLD" "$RESET" "$BOLD" "$BIN_NAME" "$RESET"
}

main "$@"
