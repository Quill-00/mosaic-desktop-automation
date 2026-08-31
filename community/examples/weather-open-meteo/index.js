const endpoint = new URL("https://api.open-meteo.com/v1/forecast");
endpoint.search = new URLSearchParams({
  latitude: "40.7128",
  longitude: "-74.0060",
  current: "temperature_2m,wind_speed_10m,relative_humidity_2m",
}).toString();

try {
  const response = await fetch(endpoint);
  if (!response.ok) throw new Error(`Open-Meteo HTTP ${response.status}`);
  const data = await response.json();
  const current = data.current ?? {};
  const metrics = [
    { label: "Temperature", value: `${current.temperature_2m ?? "-"}°C` },
    { label: "Humidity", value: `${current.relative_humidity_2m ?? "-"}%` },
    { label: "Wind", value: `${current.wind_speed_10m ?? "-"} km/h` },
  ];
  console.log(JSON.stringify({
    summary: { headline: "New York weather" },
    card: { type: "metric", title: "Weather · New York", metrics },
    items: [{ title: "Weather updated", at: new Date().toISOString() }],
  }));
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exit(1);
}
