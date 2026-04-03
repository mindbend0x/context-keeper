# ── Build stage ────────────────────────────────────────────────────────────
FROM node:22-alpine AS builder

WORKDIR /app

COPY package.json bun.lock ./
RUN npm install --frozen-lockfile || npm install

COPY . .

ARG SITE_URL
ARG SITE_BASE_URL=/
ENV SITE_URL=${SITE_URL}
ENV SITE_BASE_URL=${SITE_BASE_URL}

RUN npm run build

# ── Serve stage ───────────────────────────────────────────────────────────
FROM nginx:alpine

COPY nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=builder /app/build /usr/share/nginx/html

EXPOSE 80
