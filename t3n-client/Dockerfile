FROM node:22-alpine
WORKDIR /app
COPY package.json package-lock.json ./
RUN npm ci --production
COPY bridge.mjs ./
COPY verigate_verify.wasm ./contract/verigate_verify.wasm
ENV CONTRACT_WASM_PATH=/app/contract/verigate_verify.wasm
EXPOSE 3310
CMD ["node", "bridge.mjs"]
