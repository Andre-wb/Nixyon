# Сборка APK в контейнере. Google выпускает Android SDK и NDK для Linux только под
# x86_64, поэтому образ linux/amd64; на Mac с Apple Silicon он работает через эмуляцию.
# Эмулятор Android в контейнере не запускается: он остаётся на хосте, APK ставится через adb.

FROM --platform=linux/amd64 eclipse-temurin:17-jdk-noble@sha256:78dde986267ae7cac62fe7ef624c2a5cee2036b6822933d39fd3870618b91526 AS jdk

# Android SDK: скачивается и ставится здесь, в итоговый образ попадает только результат.
FROM --platform=linux/amd64 eclipse-temurin:17-jdk-noble@sha256:78dde986267ae7cac62fe7ef624c2a5cee2036b6822933d39fd3870618b91526 AS sdk
ARG CMDLINE_TOOLS=commandlinetools-linux-15859902_latest.zip
ARG CMDLINE_TOOLS_SHA256=4e4c464f145a7512b57d088ac6c278c03c9eea610886b35a5e0804e74eedf583
ENV ANDROID_HOME=/opt/android-sdk
RUN apt-get update \
    && apt-get install -y --no-install-recommends curl unzip ca-certificates \
    && rm -rf /var/lib/apt/lists/*
RUN curl -fsSL -o /tmp/tools.zip "https://dl.google.com/android/repository/${CMDLINE_TOOLS}" \
    && echo "${CMDLINE_TOOLS_SHA256}  /tmp/tools.zip" | sha256sum -c - \
    && mkdir -p "${ANDROID_HOME}/cmdline-tools" \
    && unzip -q /tmp/tools.zip -d "${ANDROID_HOME}/cmdline-tools" \
    && mv "${ANDROID_HOME}/cmdline-tools/cmdline-tools" "${ANDROID_HOME}/cmdline-tools/latest" \
    && rm /tmp/tools.zip
# Версии те же, что на машине разработчика и в шаблоне Tauri. build-tools 36.0.0
# по умолчанию требует Android Gradle Plugin 9.3.1, а библиотека Tauri для Android
# собирается под SDK 36: без них Gradle качает их сам при каждом запуске контейнера.
# CMake из SDK (вместе с ninja) собирает llama.cpp для плагина.
RUN yes | "${ANDROID_HOME}/cmdline-tools/latest/bin/sdkmanager" --licenses > /dev/null \
    && "${ANDROID_HOME}/cmdline-tools/latest/bin/sdkmanager" --install \
        "platform-tools" \
        "platforms;android-36" \
        "platforms;android-37.0" \
        "build-tools;36.0.0" \
        "build-tools;37.0.0" \
        "ndk;30.0.16248370" \
        "cmake;4.1.2" > /dev/null

# tauri-cli собирается отдельно, в итоговый образ попадает один бинарь.
FROM --platform=linux/amd64 rust:1.95.0-slim-trixie@sha256:e14e87345b4d5964ddcc3491d27ee046a0f23820f340c3c1e24da6880141f7c0 AS tauri-cli
RUN apt-get update \
    && apt-get install -y --no-install-recommends pkg-config libssl-dev \
    && rm -rf /var/lib/apt/lists/*
RUN cargo install tauri-cli --version 2.12.4 --locked

FROM --platform=linux/amd64 rust:1.95.0-slim-trixie@sha256:e14e87345b4d5964ddcc3491d27ee046a0f23820f340c3c1e24da6880141f7c0
# Системные библиотеки Tauri для Linux (список из документации Tauri, раздел Debian):
# без них не собирается и не проверяется clippy приложение под хост.
# python3-minimal: scripts/build-demo-apk.sh читает models.json.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
        libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev \
        libayatana-appindicator3-dev librsvg2-dev python3-minimal \
    && rm -rf /var/lib/apt/lists/*
RUN rustup target add aarch64-linux-android && rustup component add rustfmt clippy

COPY --from=jdk /opt/java/openjdk /opt/java/openjdk
COPY --from=sdk /opt/android-sdk /opt/android-sdk
COPY --from=tauri-cli /usr/local/cargo/bin/cargo-tauri /usr/local/cargo/bin/cargo-tauri

ARG UID=1000
ARG GID=1000
# На Mac GID обычно 20 (staff), в Debian он уже занят: тогда берётся существующая группа.
RUN (getent group "${GID}" > /dev/null || groupadd --gid "${GID}" builder) \
    && useradd --uid "${UID}" --gid "${GID}" --create-home builder \
    && mkdir -p /cache/cargo /cache/target /home/builder/.gradle \
    && chown -R "${UID}:${GID}" /cache /home/builder/.gradle /opt/android-sdk

ENV JAVA_HOME=/opt/java/openjdk \
    ANDROID_HOME=/opt/android-sdk \
    NDK_HOME=/opt/android-sdk/ndk/30.0.16248370 \
    CARGO_HOME=/cache/cargo \
    CARGO_TARGET_DIR=/cache/target \
    PATH=/opt/java/openjdk/bin:/opt/android-sdk/platform-tools:/usr/local/cargo/bin:${PATH}

USER builder
WORKDIR /work/nixyon-app
