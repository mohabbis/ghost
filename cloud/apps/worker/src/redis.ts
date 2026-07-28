import IORedis from "ioredis";

/**
 * Shared Redis connection for BullMQ. BullMQ requires
 * `maxRetriesPerRequest: null` on the connection it uses for blocking commands.
 */
export function createRedisConnection(): IORedis {
  const url = process.env.REDIS_URL;
  if (!url) {
    throw new Error("REDIS_URL is not set — see cloud/.env.example");
  }
  return new IORedis(url, { maxRetriesPerRequest: null });
}
