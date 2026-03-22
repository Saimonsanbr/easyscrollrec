const { invoke } = window.__TAURI_INTERNALS__;

let lastOutput = "";

// ── Expõe funções para o HTML ─────────────────────────────
window.startRecord = startRecord;
window.revealFile = revealFile;
window.goBack = goBack;
window.copyBrewCmd = copyBrewCmd;
window.recheckDeps = recheckDeps;

// ── Inicialização ─────────────────────────────────────────
window.addEventListener("DOMContentLoaded", async () => {
    await checkDeps();
    setupProgressListener();
});

// ── Verifica dependências ─────────────────────────────────
async function checkDeps() {
    try {
        const result = await invoke("check_dependencies");
        if (result.ok) {
            showScreen("main");
        } else {
            showScreen("setup");
        }
    } catch (e) {
        console.error("check_dependencies falhou:", e);
        showScreen("setup");
    }
}

async function recheckDeps() {
    await checkDeps();
}

// ── Escuta progresso vindo do Rust ────────────────────────
function setupProgressListener() {
    // no Tauri 2 o listen fica no plugin de eventos
    // por ora só logamos — o progresso ainda aparece via polling
    try {
        if (window.__TAURI__ && window.__TAURI__.event) {
            window.__TAURI__.event.listen("progress", (event) => {
                const { message } = event.payload;
                const el = document.getElementById("recording-status");
                if (el) el.textContent = message;
            });
        }
    } catch (e) {
        console.warn("listen não disponível:", e);
    }
}

// ── Inicia gravação ───────────────────────────────────────
async function startRecord() {
    const url = document.getElementById("url-input").value.trim();
    const speed = parseFloat(document.getElementById("speed-select").value);
    const fps = parseInt(document.getElementById("fps-select").value);
    let output = document.getElementById("output-input").value.trim();

    if (!url) {
        shake("url-input");
        return;
    }

    const finalUrl = url.startsWith("http") ? url : "https://" + url;

    if (!output) {
        const now = new Date();
        const hh = String(now.getHours()).padStart(2, "0");
        const mm = String(now.getMinutes()).padStart(2, "0");
        const ss = String(now.getSeconds()).padStart(2, "0");
        output = `scroll_${hh}_${mm}_${ss}.mp4`;
    }

    if (!output.endsWith(".mp4")) output += ".mp4";

    lastOutput = output;

    showScreen("recording");
    document.getElementById("recording-status").textContent = "Iniciando...";

    try {
        const result = await invoke("record", {
            url: finalUrl,
            output,
            speed,
            fps,
        });

        document.getElementById("done-path").textContent = result.output;
        showScreen("done");
    } catch (err) {
        document.getElementById("error-message").textContent =
            typeof err === "string" ? err : JSON.stringify(err);
        showScreen("error");
    }
}

// ── Mostrar arquivo no Finder ─────────────────────────────
async function revealFile() {
    try {
        await invoke("reveal_in_finder", { path: lastOutput });
    } catch (e) {
        console.error(e);
    }
}

// ── Voltar para tela principal ────────────────────────────
function goBack() {
    showScreen("main");
}

// ── Copiar comando brew ───────────────────────────────────
function copyBrewCmd() {
    navigator.clipboard.writeText("brew install ffmpeg");
    const btn = document.querySelector(".btn-copy");
    btn.textContent = "Copiado!";
    setTimeout(() => (btn.textContent = "Copiar"), 2000);
}

// ── Helpers ───────────────────────────────────────────────
function showScreen(name) {
    document.querySelectorAll(".screen").forEach((s) => s.classList.add("hidden"));
    const target = document.getElementById(`screen-${name}`);
    if (target) target.classList.remove("hidden");
}

function shake(inputId) {
    const el = document.getElementById(inputId);
    if (!el) return;
    el.style.animation = "none";
    el.offsetHeight;
    el.style.animation = "shake 0.3s ease";
    setTimeout(() => (el.style.animation = ""), 300);
}