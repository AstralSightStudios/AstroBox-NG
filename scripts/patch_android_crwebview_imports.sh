#!/usr/bin/env bash
set -euo pipefail

JAVA_ROOT="${1:-src-tauri/modules/app/gen/android/app/src/main/java}"

if [[ ! -d "${JAVA_ROOT}" ]]; then
  echo "[patch_android_crwebview_imports] skip: java root not found: ${JAVA_ROOT}"
  exit 0
fi

patch_kotlin_imports() {
  local file_path="$1"

  perl -0pi -e '
    s/^import android\.webkit\.WebView$/import org.chromium.android_crwebview.webkit.WebView/mg;
    s/^import android\.webkit\.WebViewClient$/import org.chromium.android_crwebview.webkit.WebViewClient/mg;
    s/^import android\.webkit\.WebChromeClient$/import org.chromium.android_crwebview.webkit.WebChromeClient/mg;
    s/^import android\.webkit\.WebResourceRequest$/import org.chromium.android_crwebview.webkit.WebResourceRequest/mg;
    s/^import android\.webkit\.WebResourceResponse$/import org.chromium.android_crwebview.webkit.WebResourceResponse/mg;
    s/^import android\.webkit\.WebResourceError$/import org.chromium.android_crwebview.webkit.WebResourceError/mg;
    s/^import android\.webkit\.CookieManager$/import org.chromium.android_crwebview.webkit.CookieManager/mg;
    s/^import android\.webkit\.ValueCallback$/import org.chromium.android_crwebview.webkit.ValueCallback/mg;
    s/^import android\.webkit\.PermissionRequest$/import org.chromium.android_crwebview.webkit.PermissionRequest/mg;
    s/^import android\.webkit\.GeolocationPermissions$/import org.chromium.android_crwebview.webkit.GeolocationPermissions/mg;
    s/^import android\.webkit\.JsResult$/import org.chromium.android_crwebview.webkit.JsResult/mg;
    s/^import android\.webkit\.JsPromptResult$/import org.chromium.android_crwebview.webkit.JsPromptResult/mg;
    s/^import android\.webkit\.JavascriptInterface$/import org.chromium.android_crwebview.webkit.JavascriptInterface/mg;
    s/^import android\.webkit\.\*$/import org.chromium.android_crwebview.webkit.*/mg;
    s/\(Landroid\/webkit\/WebView;\)/\(Lorg\/chromium\/android_crwebview\/webkit\/WebView;\)/g;
    s/\(Landroid\/webkit\/WebViewClient;\)/\(Lorg\/chromium\/android_crwebview\/webkit\/WebViewClient;\)/g;
    s/\(Landroid\/webkit\/WebChromeClient;\)/\(Lorg\/chromium\/android_crwebview\/webkit\/WebChromeClient;\)/g;
  ' "${file_path}"
}

# Patch all generated Kotlin sources and app entry Kotlin sources.
while IFS= read -r file; do
  patch_kotlin_imports "${file}"
done < <(find "${JAVA_ROOT}" -type f -name '*.kt')

# RustWebChromeClient still relies on framework MimeTypeMap.
# Ensure the import exists after bulk android.webkit -> crwebview replacements.
find "${JAVA_ROOT}" -type f -path '*/RustWebChromeClient.kt' | while read -r file; do
  perl -0pi -e '
    if ($_ =~ /MimeTypeMap\.getSingleton\(\)/ && $_ !~ /^import android\.webkit\.MimeTypeMap$/m) {
      s/^(package [^\n]+\n)/$1\nimport android.webkit.MimeTypeMap\n/m;
    }
  ' "${file}"
done

# Remove previous SystemWebView fallback rewrites so runtime stays on crwebview.
find "${JAVA_ROOT}" -type f -path '*/WryActivity.kt' | while read -r file; do
  perl -0pi -e '
    s/^\s*import android\.webkit\.WebView as SystemWebView\s*\n//mg;
    s/\bSystemWebView\.getCurrentWebViewPackage\(\)/WebView.getCurrentWebViewPackage()/g;
  ' "${file}"
done

ANDROIDX_WEBKIT_DIR="${JAVA_ROOT}/androidx/webkit"
mkdir -p "${ANDROIDX_WEBKIT_DIR}"

cat > "${ANDROIDX_WEBKIT_DIR}/WebViewCompat.java" <<'JAVA'
package androidx.webkit;

import java.util.Set;

public final class WebViewCompat {
    public interface ScriptHandler {
        void remove();
    }

    private static final ScriptHandler NO_OP_SCRIPT_HANDLER = new ScriptHandler() {
        @Override
        public void remove() {
            // No-op.
        }
    };

    private WebViewCompat() {}

    public static ScriptHandler addDocumentStartJavaScript(
        Object webView,
        String script,
        Set<String> allowedOriginRules
    ) {
        // crwebview path: document-start script injection is not wired yet.
        // RustWebViewClient will fallback to onPageStarted injection.
        return NO_OP_SCRIPT_HANDLER;
    }
}
JAVA

cat > "${ANDROIDX_WEBKIT_DIR}/WebViewFeature.java" <<'JAVA'
package androidx.webkit;

public final class WebViewFeature {
    public static final String DOCUMENT_START_SCRIPT = "DOCUMENT_START_SCRIPT";

    private WebViewFeature() {}

    public static boolean isFeatureSupported(String feature) {
        // crwebview path: fall back to classic injection strategy in RustWebViewClient.
        return false;
    }
}
JAVA

cat > "${ANDROIDX_WEBKIT_DIR}/WebViewAssetLoader.java" <<'JAVA'
package androidx.webkit;

import android.content.Context;
import android.content.res.AssetManager;
import android.net.Uri;
import org.chromium.android_crwebview.webkit.WebResourceResponse;

import java.io.IOException;
import java.io.InputStream;
import java.net.URLConnection;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

public final class WebViewAssetLoader {
    private final String domain;
    private final List<PathHandlerEntry> handlers;

    private WebViewAssetLoader(String domain, List<PathHandlerEntry> handlers) {
        this.domain = domain;
        handlers.sort(Comparator.comparingInt((PathHandlerEntry e) -> e.prefix.length()).reversed());
        this.handlers = handlers;
    }

    public WebResourceResponse shouldInterceptRequest(Uri url) {
        if (url == null) {
            return null;
        }

        final String host = url.getHost();
        if (host == null || !host.equalsIgnoreCase(domain)) {
            return null;
        }

        String path = url.getPath();
        if (path == null || path.isEmpty()) {
            path = "/";
        }

        for (PathHandlerEntry entry : handlers) {
            if (!path.startsWith(entry.prefix)) {
                continue;
            }

            String relativePath = path.substring(entry.prefix.length());
            while (relativePath.startsWith("/")) {
                relativePath = relativePath.substring(1);
            }

            WebResourceResponse response = entry.handler.handle(relativePath);
            if (response != null) {
                return response;
            }
        }

        return null;
    }

    public interface PathHandler {
        WebResourceResponse handle(String path);
    }

    public static final class Builder {
        private String domain = "localhost";
        private final List<PathHandlerEntry> handlers = new ArrayList<>();

        public Builder setDomain(String domain) {
            if (domain != null && !domain.isEmpty()) {
                this.domain = domain;
            }
            return this;
        }

        public Builder addPathHandler(String path, PathHandler handler) {
            if (handler == null) {
                return this;
            }
            handlers.add(new PathHandlerEntry(normalizePrefix(path), handler));
            return this;
        }

        public WebViewAssetLoader build() {
            return new WebViewAssetLoader(domain, new ArrayList<>(handlers));
        }

        private static String normalizePrefix(String path) {
            if (path == null || path.isEmpty()) {
                return "/";
            }
            String normalized = path;
            if (!normalized.startsWith("/")) {
                normalized = "/" + normalized;
            }
            return normalized;
        }
    }

    public static final class AssetsPathHandler implements PathHandler {
        private final AssetManager assets;

        public AssetsPathHandler(Context context) {
            this.assets = context.getAssets();
        }

        @Override
        public WebResourceResponse handle(String path) {
            final String safePath = sanitize(path);
            if (safePath == null) {
                return null;
            }

            for (String candidate : candidatesFor(safePath)) {
                try {
                    InputStream inputStream = assets.open(candidate, AssetManager.ACCESS_STREAMING);
                    String mimeType = detectMimeType(candidate);
                    String encoding = isTextType(mimeType) ? "utf-8" : null;
                    return new WebResourceResponse(mimeType, encoding, inputStream);
                } catch (IOException ignored) {
                    // Continue trying fallbacks.
                }
            }

            return null;
        }

        private static String sanitize(String path) {
            String normalized = path == null ? "" : path;
            while (normalized.startsWith("/")) {
                normalized = normalized.substring(1);
            }
            if (normalized.contains("..")) {
                return null;
            }
            return normalized;
        }

        private static List<String> candidatesFor(String path) {
            List<String> candidates = new ArrayList<>();
            if (path.isEmpty()) {
                candidates.add("index.html");
                return candidates;
            }

            candidates.add(path);
            if (path.endsWith("/")) {
                candidates.add(path + "index.html");
            }
            return candidates;
        }

        private static String detectMimeType(String path) {
            if (path.endsWith(".js") || path.endsWith(".mjs")) {
                return "text/javascript";
            }
            if (path.endsWith(".css")) {
                return "text/css";
            }
            if (path.endsWith(".html") || path.endsWith(".htm")) {
                return "text/html";
            }
            if (path.endsWith(".json")) {
                return "application/json";
            }
            if (path.endsWith(".svg")) {
                return "image/svg+xml";
            }
            if (path.endsWith(".wasm")) {
                return "application/wasm";
            }
            if (path.endsWith(".png")) {
                return "image/png";
            }
            if (path.endsWith(".jpg") || path.endsWith(".jpeg")) {
                return "image/jpeg";
            }
            if (path.endsWith(".webp")) {
                return "image/webp";
            }
            String guessed = URLConnection.guessContentTypeFromName(path);
            return guessed != null ? guessed : "application/octet-stream";
        }

        private static boolean isTextType(String mimeType) {
            return mimeType.startsWith("text/")
                || "application/json".equals(mimeType)
                || "application/javascript".equals(mimeType)
                || "text/javascript".equals(mimeType)
                || "application/xml".equals(mimeType);
        }
    }

    private static final class PathHandlerEntry {
        private final String prefix;
        private final PathHandler handler;

        private PathHandlerEntry(String prefix, PathHandler handler) {
            this.prefix = prefix;
            this.handler = handler;
        }
    }
}
JAVA

echo "[patch_android_crwebview_imports] done"
