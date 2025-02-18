import React from "react"
import { Route, Routes } from "react-router-dom"
import { WelcomeView } from "../../views/WelcomeView/WelcomeView"

export const Router = () => {
  return (
    <Routes>
      <Route path={"/"} element={<WelcomeView />} />
    </Routes>
  )
}
