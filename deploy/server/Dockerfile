# Package the matching Linux Server output; compilation remains a separate stage.
# Keep Codeg's Node 24 / Debian bookworm / OfficeCLI ICU runtime baseline.
FROM node:24-bookworm-slim
ARG SERVER_PACKAGE=out/server/linux-x64
RUN test "$(uname -m)" = x86_64 \
    && apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates git openssh-client curl python3 python3-pip libicu72 tini \
    && rm -rf /var/lib/apt/lists/* \
    && mkdir -p /opt/workbench /data /workspace /home/node/.local/bin \
    && chown -R node:node /data /workspace /home/node
COPY --chown=root:root ${SERVER_PACKAGE}/ /opt/workbench/
COPY deploy/server/verify-inputs.mjs /usr/local/lib/workbench-verify-inputs.mjs
RUN node /usr/local/lib/workbench-verify-inputs.mjs /opt/workbench
ENV HOME=/home/node \
    SHELL=/bin/bash \
    NPM_CONFIG_PREFIX=/home/node/.local \
    PATH=/home/node/.local/bin:/opt/workbench:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin \
    ROVAI_SERVER_UPDATE_MODE=container-image
LABEL org.opencontainers.image.title="Codeg Rovai Workbench Server" \
    org.opencontainers.image.source="https://github.com/knight-bili/codeg-rovai-workbench" \
    app.codegrovai.target-version="V1.0.0" \
    app.codegrovai.qualification="source-authored-not-verified"
USER node:node
WORKDIR /workspace
EXPOSE 8767
VOLUME ["/data", "/workspace", "/home/node"]
STOPSIGNAL SIGTERM
ENTRYPOINT ["/usr/bin/tini", "--", "/opt/workbench/rovai-server", "--data-dir", "/data", "--listen", "0.0.0.0:8767", "--allow-insecure-lan"]
