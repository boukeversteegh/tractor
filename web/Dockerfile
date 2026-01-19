FROM nginx:alpine

# Copy built assets to nginx html directory
COPY dist/ /usr/share/nginx/html/

# Copy custom nginx config for SPA routing and proper MIME types
COPY nginx.conf /etc/nginx/conf.d/default.conf

EXPOSE 8080

CMD ["nginx", "-g", "daemon off;"]
