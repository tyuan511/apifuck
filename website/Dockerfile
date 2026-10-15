FROM node:22-alpine AS build
WORKDIR /site
COPY src ./src
COPY public ./public
COPY scripts/build.mjs ./scripts/build.mjs
ARG SITE_REVISION=local
ENV SITE_REVISION=$SITE_REVISION
RUN node scripts/build.mjs

FROM nginx:stable-alpine
COPY deploy/nginx.conf /etc/nginx/conf.d/default.conf
COPY --from=build /site/dist /usr/share/nginx/html
EXPOSE 8080
HEALTHCHECK --interval=15s --timeout=3s --retries=4 CMD wget -q -O /dev/null http://127.0.0.1:8080/health.json || exit 1
