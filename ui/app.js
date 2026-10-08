const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const appWindow = window.__TAURI__.window.getCurrentWindow();

const TABS = ["launch", "routes", "config"];

const urlInput = document.querySelector("#url");
const browserSelect = document.querySelector("#browser");
const profileSelect = document.querySelector("#profile");
const routeUrlInput = document.querySelector("#route-url");
const routeBrowserSelect = document.querySelector("#route-browser");
const routeProfileSelect = document.querySelector("#route-profile");
const labelInput = document.querySelector("#label");
const presetList = document.querySelector("#presets");
const defaultBrowserSelect = document.querySelector("#default-browser");
const defaultProfileSelect = document.querySelector("#default-profile");
const configBrowserList = document.querySelector("#config-browsers");
const defaultStatus = document.querySelector("#default-status");
const installOutput = document.querySelector("#install-output");
const error = document.querySelector("#error");

let browsers = [];
let presets = [];
let defaultBrowserId = "";
let defaultProfileId = "";
let profileNames = {};

function showError(message) {
  error.hidden = !message;
  error.textContent = message || "";
}

function showTab(name, persist = true) {
  if (!TABS.includes(name)) name = "launch";
  for (const tab of TABS) {
    document.querySelector(`#panel-${tab}`).hidden = tab !== name;
    document
      .querySelector(`.tab[data-tab="${tab}"]`)
      .classList.toggle("active", tab === name);
  }
  if (persist) localStorage.setItem("porter.tab", name);
}

function fillSelect(select, entries, current, emptyLabel = "None yet") {
  select.replaceChildren();
  if (entries.length === 0) {
    const option = document.createElement("option");
    option.value = "";
    option.textContent = emptyLabel;
    select.append(option);
    return;
  }
  for (const entry of entries) {
    const option = document.createElement("option");
    option.value = entry.value;
    option.textContent = entry.label;
    select.append(option);
  }
  if (entries.some((entry) => entry.value === current)) {
    select.value = current;
  }
}

function browserEntries() {
  return browsers.map((browser) => ({ value: browser.id, label: browser.name }));
}

async function profileEntries(browserId) {
  const browser = browsers.find((item) => item.id === browserId);
  if (!browser || browser.kind === "safari") return [];
  const profiles = await invoke("list_profiles", { browserId: browser.id });
  profileNames[browser.id] = Object.fromEntries(
    profiles.map((profile) => [profile.id, profile.name]),
  );
  return profiles.map((profile) => ({ value: profile.id, label: profile.name }));
}

function profileNameOf(preset) {
  if (!preset.profile) return "";
  return profileNames[preset.browserId]?.[preset.profile] || preset.profile;
}

async function fillProfiles(select, browserId, current = select.value) {
  const browser = browsers.find((item) => item.id === browserId);
  select.disabled = !browser || browser.kind === "safari";
  if (!browser || browser.kind === "safari") {
    fillSelect(select, [], "—", "—");
    return;
  }
  const entries = await profileEntries(browser.id);
  fillSelect(select, entries, current, "No profiles");
}

function selectedBrowser() {
  return browsers.find((browser) => browser.id === browserSelect.value);
}

function originOf(url) {
  return (
    url
      .replace(/^[a-z]+:\/\//i, "")
      .replace(/^www\./, "")
      .split(/[/?#]/)[0] || url
  );
}

function renderPresets() {
  presetList.replaceChildren();
  if (presets.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "None yet";
    presetList.append(empty);
    return;
  }
  for (const preset of presets) {
    const item = document.createElement("li");
    const open = document.createElement("button");
    open.type = "button";
    open.className = "name";
    const label = document.createElement("span");
    label.className = "label";
    label.textContent = preset.label;
    const meta = document.createElement("span");
    meta.className = "meta";
    const browser = browsers.find((candidate) => candidate.id === preset.browserId);
    const profileName = profileNameOf(preset);
    meta.textContent = [originOf(preset.url), browser ? browser.name : "", profileName]
      .filter(Boolean)
      .join(" · ");
    open.append(label, meta);
    open.addEventListener("click", () => run(() => invoke("launch_preset", { id: preset.id })));
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "x";
    remove.textContent = "×";
    remove.title = "Delete";
    remove.addEventListener("click", () =>
      run(async () => {
        await invoke("delete_preset", { id: preset.id });
        presets = presets.filter((entry) => entry.id !== preset.id);
        renderPresets();
      }),
    );
    item.append(open, remove);
    presetList.append(item);
  }
}

function renderConfigBrowsers() {
  configBrowserList.replaceChildren();
  if (browsers.length === 0) {
    const empty = document.createElement("p");
    empty.className = "empty";
    empty.textContent = "None yet";
    configBrowserList.append(empty);
    return;
  }
  for (const browser of browsers) {
    const item = document.createElement("li");
    if (browser.id === defaultBrowserId) {
      const badge = document.createElement("span");
      badge.className = "badge";
      badge.textContent = "default";
      item.append(badge);
    }
    const name = document.createElement("span");
    name.className = "name";
    name.textContent = browser.name;
    const remove = document.createElement("button");
    remove.type = "button";
    remove.className = "x";
    remove.textContent = "×";
    remove.title = "Remove";
    remove.addEventListener("click", () =>
      run(async () => {
        await invoke("remove_browser", { id: browser.id });
        await refresh();
      }),
    );
    item.append(name, remove);
    configBrowserList.append(item);
  }
}

function fillBrowserSelects() {
  const entries = browserEntries();
  fillSelect(browserSelect, entries, browserSelect.value);
  fillSelect(routeBrowserSelect, entries, routeBrowserSelect.value);
  fillSelect(
    defaultBrowserSelect,
    [{ value: "", label: "— Not set" }, ...entries],
    defaultBrowserId,
  );
}

async function refreshDefaultProfile() {
  defaultProfileSelect.disabled = !defaultBrowserId;
  if (!defaultBrowserId) {
    fillSelect(defaultProfileSelect, [], "—", "—");
    return;
  }
  const entries = await profileEntries(defaultBrowserId);
  fillSelect(defaultProfileSelect, entries, defaultProfileId, "No profiles");
}

async function refreshProfileNames() {
  const ids = [...new Set(presets.filter((preset) => preset.profile).map((preset) => preset.browserId))];
  await Promise.all(ids.map((browserId) => profileEntries(browserId)));
}

async function refresh() {
  const state = await invoke("get_state");
  browsers = state.browsers;
  presets = state.presets;
  defaultBrowserId = state.defaultBrowser || "";
  defaultProfileId = state.defaultProfile || "";
  profileNames = {};
  fillBrowserSelects();
  await fillProfiles(profileSelect, browserSelect.value);
  await fillProfiles(routeProfileSelect, routeBrowserSelect.value);
  await refreshDefaultProfile();
  await refreshProfileNames();
  renderPresets();
  renderConfigBrowsers();
}

async function refreshDefaultStatus() {
  try {
    const status = await invoke("default_browser_status");
    if (status === true) {
      defaultStatus.textContent = "Porter is your default browser.";
      defaultStatus.classList.add("ok");
    } else if (status === false) {
      defaultStatus.textContent = "Porter is not the default browser yet.";
      defaultStatus.classList.remove("ok");
    } else {
      defaultStatus.textContent = "Status is not reported on this system.";
      defaultStatus.classList.remove("ok");
    }
  } catch {
    defaultStatus.textContent = "Could not read the default browser status.";
    defaultStatus.classList.remove("ok");
  }
}

async function run(action) {
  showError("");
  try {
    await action();
  } catch (err) {
    showError(typeof err === "string" ? err : "Something went wrong");
  }
}

document.querySelectorAll(".tab").forEach((button) =>
  button.addEventListener("click", () => showTab(button.dataset.tab)),
);

document.querySelector("#close").addEventListener("click", () => appWindow.hide());

document.querySelector("#drag").addEventListener("mousedown", (event) => {
  if (event.target.closest("button")) return;
  appWindow.startDragging();
});

browserSelect.addEventListener("change", () => run(() => fillProfiles(profileSelect, browserSelect.value)));
routeBrowserSelect.addEventListener("change", () =>
  run(() => fillProfiles(routeProfileSelect, routeBrowserSelect.value)),
);

document.querySelector("#add").addEventListener("click", () =>
  run(async () => {
    const browser = await invoke("add_browser");
    if (!browser) return;
    await refresh();
    routeBrowserSelect.value = browser.id;
    await fillProfiles(routeProfileSelect, browser.id);
  }),
);

document.querySelector("#launch").addEventListener("click", () =>
  run(async () => {
    const browser = selectedBrowser();
    if (!browser) throw "Add a browser first";
    await invoke("launch_url", {
      browserId: browser.id,
      profile: profileSelect.value || null,
      url: urlInput.value,
    });
    await appWindow.hide();
  }),
);

document.querySelector("#save").addEventListener("click", () =>
  run(async () => {
    const browser = browsers.find((item) => item.id === routeBrowserSelect.value);
    if (!browser) throw "Add a browser first";
    const preset = await invoke("save_preset", {
      label: labelInput.value,
      url: routeUrlInput.value,
      browserId: browser.id,
      profile: routeProfileSelect.value || null,
    });
    presets.push(preset);
    labelInput.value = "";
    renderPresets();
  }),
);

defaultBrowserSelect.addEventListener("change", () =>
  run(async () => {
    await fillProfiles(defaultProfileSelect, defaultBrowserSelect.value, "");
    await invoke("set_default_browser", {
      browserId: defaultBrowserSelect.value || null,
      profile: defaultProfileSelect.value || null,
    });
    defaultBrowserId = defaultBrowserSelect.value;
    defaultProfileId = defaultProfileSelect.value;
    renderConfigBrowsers();
  }),
);

defaultProfileSelect.addEventListener("change", () =>
  run(async () => {
    await invoke("set_default_browser", {
      browserId: defaultBrowserSelect.value || null,
      profile: defaultProfileSelect.value || null,
    });
    defaultProfileId = defaultProfileSelect.value;
  }),
);

document.querySelector("#install-default").addEventListener("click", async () => {
  showError("");
  installOutput.hidden = false;
  installOutput.textContent = "Running install-default-browser.sh…";
  try {
    const text = await invoke("install_default_browser");
    installOutput.textContent = text || "Done.";
  } catch (err) {
    installOutput.textContent = typeof err === "string" ? err : "Install failed";
    showError("Could not make Porter the default browser");
  }
  await refreshDefaultStatus();
});

urlInput.addEventListener("keydown", (event) => {
  if (event.key === "Enter") document.querySelector("#launch").click();
});

[routeUrlInput, labelInput].forEach((input) =>
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") document.querySelector("#save").click();
  }),
);

async function fillIncomingUrl(url) {
  showTab("launch");
  urlInput.value = url;
  urlInput.focus();
  urlInput.select();
  await fillProfiles(profileSelect, browserSelect.value);
  invoke("auto_launch_url", {
    url,
    browserId: browserSelect.value || null,
    profile: profileSelect.value || null,
  }).catch((err) => showError(String(err)));
}

listen("open-url", (event) => fillIncomingUrl(event.payload));

async function takePending() {
  const pending = await invoke("take_pending_url");
  if (pending) fillIncomingUrl(pending);
}

showTab(localStorage.getItem("porter.tab") || "launch", false);
refresh().catch((err) => showError(String(err)));
refreshDefaultStatus().catch(() => {});
takePending().catch(() => {});
