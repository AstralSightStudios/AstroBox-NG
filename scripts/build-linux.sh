#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(dirname "$SCRIPT_DIR")"
BUNDLE_DIR="$PROJECT_ROOT/src-tauri/target/release/bundle"
OUTPUT_DIR="$PROJECT_ROOT/dist/linux"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
CYAN='\033[0;36m'
BOLD='\033[1m'
NC='\033[0m'

info()  { echo -e "${CYAN}[INFO]${NC} $*"; }
ok()    { echo -e "${GREEN}[ OK ]${NC} $*"; }
warn()  { echo -e "${YELLOW}[WARN]${NC} $*"; }
err()   { echo -e "${RED}[ERR!]${NC} $*"; }

# ============================================================
# 包名修改函数
# ============================================================

repackage_deb() {
    local abs_deb="$1"
    local abs_out="$2"
    local temp_dir
    temp_dir=$(mktemp -d)

    echo "  Repackaging $(basename "$abs_deb")..."

    dpkg-deb -R "$abs_deb" "$temp_dir"

    if [ -f "$temp_dir/DEBIAN/control" ]; then
        sed -i 's/^Package: astro-box$/Package: astrobox-ng/' "$temp_dir/DEBIAN/control"
        echo "    Modified Package: astro-box -> astrobox-ng"
    fi

    dpkg-deb -b --root-owner-group "$temp_dir" "$abs_out"
    rm -rf "$temp_dir" "$abs_deb"
}

repackage_rpm() {
    local abs_rpm="$1"
    local abs_out="$2"
    local build_dir
    build_dir=$(mktemp -d)
    local spec_file="$build_dir/astrobox-ng.spec"

    echo "  Repackaging $(basename "$abs_rpm")..."

    local version release
    version=$(rpm -q --qf '%{VERSION}' -p "$abs_rpm" 2>/dev/null || echo "2.0.0")
    release=$(rpm -q --qf '%{RELEASE}' -p "$abs_rpm" 2>/dev/null || echo "1")

    cp "$abs_rpm" "$build_dir/"
    cd "$build_dir"
    rpm2archive "$(basename "$abs_rpm")" -f cpio | gunzip | cpio -idm 2>/dev/null || true

    local content_dir="$build_dir/content"
    mkdir -p "$content_dir"
    for item in "$build_dir"/*; do
        local name
        name=$(basename "$item")
        case "$name" in
            content|astrobox-ng.spec|rpmbuild|*.rpm|SPECPARTS) continue ;;
            *) mv "$item" "$content_dir/" ;;
        esac
    done

    cat > "$spec_file" << EOF
Name: astrobox-ng
Version: $version
Release: $release
Summary: AstroBox - Wearable Device Toolbox
License: AGPL-3.0
Group: Applications/System

%description
AstroBox is a multifunctional toolbox designed for Xiaomi Vela wearable devices.

%install
mkdir -p %{buildroot}
cp -a $content_dir/* %{buildroot}/

%files
/*
EOF

    mkdir -p "$build_dir/rpmbuild"/{BUILD,RPMS,SOURCES,SPECS,SRPMS}

    rpmbuild --define "_topdir $build_dir/rpmbuild" \
             -bb "$spec_file" 2>/dev/null || {
        echo "    Warning: rpmbuild failed, falling back to simple rename"
        cp "$abs_rpm" "$abs_out"
        rm -rf "$build_dir"
        return
    }

    local new_rpm
    new_rpm=$(find "$build_dir/rpmbuild/RPMS" -name "astrobox-ng-*.rpm" -type f | head -1)
    if [ -n "$new_rpm" ]; then
        mv "$new_rpm" "$abs_out"
        rm -f "$abs_rpm"
    else
        echo "    Warning: Could not find repackaged rpm, falling back to simple rename"
        cp "$abs_rpm" "$abs_out"
    fi

    rm -rf "$build_dir"
}

step_repackage() {
    echo ""
    info "=========================================="
    info "Step 2/3: 重命名 Linux 包 (astrobox-ng)"
    info "=========================================="

    if [ -d "$BUNDLE_DIR/deb" ]; then
        info "处理 deb 包..."
        cd "$BUNDLE_DIR/deb"
        for deb_file in AstroBox_*.deb; do
            [ -f "$deb_file" ] || continue
            local version arch
            version=$(echo "$deb_file" | grep -oP '\d+\.\d+\.\d+')
            arch=$(echo "$deb_file" | grep -oP 'amd64|arm64|armhf')
            [ -n "$version" ] && [ -n "$arch" ] || continue
            repackage_deb "$BUNDLE_DIR/deb/$deb_file" "$BUNDLE_DIR/deb/astrobox-ng_${version}_${arch}.deb"
        done
        for dir in AstroBox_*; do
            [ -d "$dir" ] || continue
            local new_dir
            new_dir=$(echo "$dir" | sed 's/AstroBox_/astrobox-ng_/')
            echo "  Renaming directory $dir -> $new_dir"
            rm -rf "$new_dir"
            mv "$dir" "$new_dir"
        done
    fi

    if [ -d "$BUNDLE_DIR/rpm" ]; then
        info "处理 rpm 包..."
        cd "$BUNDLE_DIR/rpm"
        for rpm_file in AstroBox-*.rpm; do
            [ -f "$rpm_file" ] || continue
            local version release arch
            version=$(echo "$rpm_file" | grep -oP '\d+\.\d+\.\d+')
            release=$(echo "$rpm_file" | grep -oP '\d+\.\d+\.\d+-\K\d+')
            arch=$(echo "$rpm_file" | grep -oP 'x86_64|aarch64|armv7hl')
            [ -n "$version" ] && [ -n "$arch" ] || continue
            repackage_rpm "$BUNDLE_DIR/rpm/$rpm_file" "$BUNDLE_DIR/rpm/astrobox-ng-${version}-${release:-1}.${arch}.rpm"
        done
        for dir in AstroBox-*; do
            [ -d "$dir" ] || continue
            local new_dir
            new_dir=$(echo "$dir" | sed 's/AstroBox-/astrobox-ng-/')
            echo "  Renaming directory $dir -> $new_dir"
            rm -rf "$new_dir"
            mv "$dir" "$new_dir"
        done
    fi

if [ -d "$BUNDLE_DIR/appimage" ]; then
    info "清理 AppImage 产物..."
    rm -f "$BUNDLE_DIR"/appimage/AstroBox_*.AppImage
fi

    ok "包重命名完成"
}

# ============================================================
# 交互菜单
# ============================================================

show_menu() {
    echo ""
    echo -e "${BOLD}========================================${NC}"
    echo -e "${BOLD}  AstroBox-NG Linux 打包工具${NC}"
    echo -e "${BOLD}========================================${NC}"
    echo ""
    echo "请选择要构建的包类型（可多选，用空格分隔）："
    echo ""
    echo -e "  ${BOLD}1)${NC} deb      - Debian/Ubuntu 包"
    echo -e "  ${BOLD}2)${NC} rpm      - Fedora/RHEL 包"
    echo -e "  ${BOLD}3)${NC} arch     - Arch Linux 包 (prebuilt，快速)"
    echo -e "  ${BOLD}4)${NC} arch-full - Arch Linux 包 (从源码完整编译)"
    echo ""
    echo -e "  ${BOLD}a)${NC} 全部构建 (deb + rpm)"
    echo -e "  ${BOLD}q)${NC} 退出"
    echo ""
}

prompt_selection() {
    local prompt="$1"
    shift
    local selected=()

    echo -e "$prompt"

    while true; do
        echo -n "> "
        read -r input

        if [[ "$input" == "q" || "$input" == "Q" ]]; then
            echo ""
            return 1
        fi

        if [[ -z "$input" ]]; then
            break
        fi

        IFS=' ' read -ra tokens <<< "$input"
        selected=()

        for token in "${tokens[@]}"; do
            case "$token" in
                1|deb)       selected+=("deb") ;;
                2|rpm)       selected+=("rpm") ;;
                3|arch)      selected+=("arch") ;;
                4|arch-full) selected+=("arch-full") ;;
                a|A)         selected=("deb" "rpm") ;;
                *)
                    err "无效选项: $token"
                    continue ;;
            esac
        done

        if [ ${#selected[@]} -gt 0 ]; then
            SELECTED_TARGETS=("${selected[@]}")
            return 0
        fi
    done

    SELECTED_TARGETS=()
    return 0
}

# ============================================================
# 依赖检查
# ============================================================

check_deps() {
    local missing=()

    command -v pnpm >/dev/null 2>&1 || missing+=("pnpm")

    if [[ " ${SELECTED_TARGETS[*]} " =~ " deb " ]]; then
        command -v dpkg-deb >/dev/null 2>&1 || missing+=("dpkg-deb")
    fi

    if [[ " ${SELECTED_TARGETS[*]} " =~ " rpm " ]]; then
        command -v rpmbuild >/dev/null 2>&1 || missing+=("rpmbuild")
        command -v rpm2archive >/dev/null 2>&1 || missing+=("rpm2archive")
    fi

    if [[ " ${SELECTED_TARGETS[*]} " =~ " arch " || " ${SELECTED_TARGETS[*]} " =~ " arch-full " ]]; then
        command -v makepkg >/dev/null 2>&1 || missing+=("makepkg")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        err "缺少以下依赖: ${missing[*]}"
        exit 1
    fi
}

# ============================================================
# 构建步骤
# ============================================================

step_sync() {
    echo ""
    info "=========================================="
    info "Step 0/3: 同步子仓库 (abtools sync --private)"
    info "=========================================="

    cd "$PROJECT_ROOT"
    python3 abtools.py sync --private

    ok "子仓库同步完成"
}

step_build() {
    echo ""
    info "=========================================="
    info "Step 1/3: 运行 pnpm tauri build"
    info "=========================================="

    cd "$PROJECT_ROOT"

    local bundles=()
    local has_native_bundle=false
    for target in "${SELECTED_TARGETS[@]}"; do
        case "$target" in
            deb)  bundles+=("deb"); has_native_bundle=true ;;
            rpm)  bundles+=("rpm"); has_native_bundle=true ;;
        esac
    done

    if [ "$has_native_bundle" = true ]; then
        local bundle_arg
        bundle_arg=$(IFS=,; echo "${bundles[*]}")
        pnpm tauri build --bundles "$bundle_arg"
    else
        info "没有需要 native bundler 的目标，仅编译项目（--no-bundle）"
        pnpm tauri build --no-bundle
    fi

    ok "Tauri 构建完成"
}

build_deb() {
    echo ""
    info "--- 收集 deb 包 ---"

    local deb_files
    deb_files=$(find "$BUNDLE_DIR/deb" -name "astrobox-ng_*.deb" -type f 2>/dev/null)

    if [ -z "$deb_files" ]; then
        warn "未找到 deb 包"
        return
    fi

    mkdir -p "$OUTPUT_DIR"
    while IFS= read -r f; do
        cp "$f" "$OUTPUT_DIR/"
        ok "$OUTPUT_DIR/$(basename "$f")"
    done <<< "$deb_files"
}

build_rpm() {
    echo ""
    info "--- 收集 rpm 包 ---"

    local rpm_files
    rpm_files=$(find "$BUNDLE_DIR/rpm" -name "astrobox-ng-*.rpm" -type f 2>/dev/null)

    if [ -z "$rpm_files" ]; then
        warn "未找到 rpm 包"
        return
    fi

    mkdir -p "$OUTPUT_DIR"
    while IFS= read -r f; do
        cp "$f" "$OUTPUT_DIR/"
        ok "$OUTPUT_DIR/$(basename "$f")"
    done <<< "$rpm_files"
}

build_arch() {
    local mode="$1"
    echo ""
    info "--- 构建 Arch Linux 包 ($mode) ---"

    "$SCRIPT_DIR/archpkg/build.sh" "$mode"

    mkdir -p "$OUTPUT_DIR"
    local arch_pkgs
    arch_pkgs=$(find "$SCRIPT_DIR/archpkg" -maxdepth 1 -name "astrobox-ng-*.pkg.tar.zst" -type f 2>/dev/null)
    if [ -n "$arch_pkgs" ]; then
        while IFS= read -r f; do
            cp "$f" "$OUTPUT_DIR/"
            ok "$OUTPUT_DIR/$(basename "$f")"
        done <<< "$arch_pkgs"
    fi
}

# ============================================================
# 主流程
# ============================================================

show_menu
prompt_selection "输入选项编号（如: 1 2 3 或 deb rpm arch）："
selection_rc=$?

if [ $selection_rc -ne 0 ] || [ ${#SELECTED_TARGETS[@]} -eq 0 ]; then
    info "已取消"
    exit 0
fi

echo ""
info "已选择: ${SELECTED_TARGETS[*]}"
check_deps

echo ""
info "=========================================="
info "开始构建"
info "=========================================="

step_sync
step_build

# arch prebuilt 必须在重命名之前执行，因为 PKGBUILD 引用了 AstroBox_* 路径
for target in "${SELECTED_TARGETS[@]}"; do
    case "$target" in
        arch)      build_arch prebuilt ;;
        arch-full) build_arch full ;;
    esac
    done

# 只有选中的目标包含 deb 或 rpm 时才执行重命名
has_native_target=false
for target in "${SELECTED_TARGETS[@]}"; do
    case "$target" in
        deb|rpm) has_native_target=true; break ;;
    esac
done

if [ "$has_native_target" = true ]; then
    step_repackage
fi

mkdir -p "$OUTPUT_DIR"

for target in "${SELECTED_TARGETS[@]}"; do
    case "$target" in
        deb)       build_deb ;;
        rpm)       build_rpm ;;
    esac
done

echo ""
echo -e "${BOLD}========================================${NC}"
ok "全部完成！输出目录: $OUTPUT_DIR"
echo -e "${BOLD}========================================${NC}"
echo ""
ls -lh "$OUTPUT_DIR"/ 2>/dev/null
