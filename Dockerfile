# Use a lightweight Nginx image to serve static files
FROM nginx:alpine

# Copy the static marketing website files to the Nginx public directory
COPY ./public /usr/share/nginx/html

# Expose port 80 for container traffic
EXPOSE 80

# Start Nginx
CMD ["nginx", "-g", "daemon off;"]
