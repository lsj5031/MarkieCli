#!/usr/bin/env sh
set -eu

REPO="${REPO:-lsj5031/MarkieCli}"
BIN_NAME="markie"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.local/bin}"
VERSION_INPUT="${MARKIE_VERSION:-latest}"

detect_target() {
    os="$(uname -s)"
    arch="$(uname -m)"

    case "$os" in
        Linux) os_part="unknown-linux-gnu" ;;
        Darwin) os_part="apple-darwin" ;;
        *)
            echo "Unsupported OS: $os" >&2
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64 | amd64) arch_part="x86_64" ;;
        aarch64 | arm64) arch_part="aarch64" ;;
        *)
            echo "Unsupported architecture: $arch" >&2
            exit 1
            ;;
    esac

    printf "%s-%s" "$arch_part" "$os_part"
}

resolve_tag() {
    case "$VERSION_INPUT" in
        latest)
            tag="$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
                | sed -n 's/.*"tag_name":[[:space:]]*"\([^"]*\)".*/\1/p' \
                | head -n1)"
            if [ -z "$tag" ]; then
                echo "Failed to resolve latest release tag from GitHub API." >&2
                exit 1
            fi
            printf "%s" "$tag"
            ;;
        v*) printf "%s" "$VERSION_INPUT" ;;
        *) printf "v%s" "$VERSION_INPUT" ;;
    esac
}

target="$(detect_target)"
tag="$(resolve_tag)"
asset="${BIN_NAME}-${target}.tar.gz"
url="https://github.com/$REPO/releases/download/$tag/$asset"

tmpdir="$(mktemp -d)"
trap 'rm -rf "$tmpdir"' EXIT INT TERM

echo "Downloading $url"
curl -fL "$url" -o "$tmpdir/$asset"
tar -xzf "$tmpdir/$asset" -C "$tmpdir"

src_bin="$tmpdir/$BIN_NAME"
if [ ! -f "$src_bin" ]; then
    src_bin="$(find "$tmpdir" -type f -name "$BIN_NAME" | head -n1 || true)"
fi

if [ -z "${src_bin:-}" ] || [ ! -f "$src_bin" ]; then
    echo "Could not find '$BIN_NAME' inside downloaded archive." >&2
    exit 1
fi

mkdir -p "$INSTALL_DIR"
install -m 755 "$src_bin" "$INSTALL_DIR/$BIN_NAME"

echo "Installed $BIN_NAME to $INSTALL_DIR/$BIN_NAME"
case ":$PATH:" in
    *":$INSTALL_DIR:"*) ;;
    *)
        echo "Note: $INSTALL_DIR is not in PATH."
        echo "Add it with: export PATH=\"$INSTALL_DIR:\$PATH\""
        ;;
esac
