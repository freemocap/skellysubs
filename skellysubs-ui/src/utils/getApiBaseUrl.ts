export const getApiBaseUrl = () => {
  // Use Vite's environment variables (if using Create React App, use process.env.REACT_APP_API_BASE_URL)
  return (
    import.meta.env.VITE_API_BASE_URL ||
    process.env.REACT_APP_API_BASE_URL ||
    "http://localhost:8080"
  )
}
