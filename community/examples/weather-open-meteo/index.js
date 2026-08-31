const endpoint = new URL("https://api.open-meteo.com/v1/forecast");
endpoint.search = new URLSearchParams({
  latitude: "39.9",
  longitude: "116.4",
  current: "temperature_2m,wind_speed_10m,relative_humidity_2m",
}).toString();

try {
  const response = await fetch(endpoint);
  if (!response.ok) throw new Error(`Open-Meteo HTTP ${response.status}`);
  const data = await response.json();
  const current = data.current ?? {};
  const metrics = [
    { label: "温度", value: `${current.temperature_2m ?? "-"}°C` },
    { label: "湿度", value: `${current.relative_humidity_2m ?? "-"}%` },
    { label: "风速", value: `${current.wind_speed_10m ?? "-"} km/h` },
  ];
  console.log(JSON.stringify({
    summary: { headline: "北京天气" },
    card: { type: "metric", title: "天气 · 北京", metrics },
    items: [{ title: "天气更新", at: new Date().toISOString() }],
  }));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
