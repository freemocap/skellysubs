import { useAppSelector } from "../../app/hooks"

export const WebsocketDataDisplay = () => {
  const websocketData = useAppSelector(state => state.websocket.dataBySessionId)

  return (
    <div>
      {Object.keys(websocketData).map(sessionId => (
        <div key={sessionId}>
          <h3>Session ID: {sessionId}</h3>
          <ul>
            {websocketData[sessionId].map((data: any, index: number) => (
              <li key={index}>{JSON.stringify(data)}</li>
            ))}
          </ul>
        </div>
      ))}
    </div>
  )
}
