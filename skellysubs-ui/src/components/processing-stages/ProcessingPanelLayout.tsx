import {Box, BoxProps, Button, CircularProgress} from "@mui/material"
import type React from "react"

type ProcessingPanelLayoutProps = BoxProps & {
    children: React.ReactNode
}

export const ProcessingPanelLayout = ({
                                          children,
                                          ...props
                                      }: ProcessingPanelLayoutProps) => (
    <Box
        sx={{
            m: 3,
            p: 3,
            display: "flex",
            flexDirection: "column",
            alignItems: "center",
            borderStyle: "solid",
            borderColor: "#990",
            borderWidth: "1px",
            borderRadius: 2,
        }}
        {...props}
    >
        {children}
    </Box>
)

export const ProcessingButton = ({
                                     status,
                                     isReady,
                                     label,
                                     onClick,
                                 }: {
    status: string
    isReady: boolean
    label: string
    onClick: () => void
}) => (
    <Button
        variant="contained"
        color="secondary"
        sx={{ m: 2, position: "relative" }}
        onClick={onClick}
        disabled={!isReady || status === "processing"}
    >
        {status === "processing" ? "Processing..." : label}
        {status === "processing" && (
            <CircularProgress
                size={24}
                sx={{
                    position: "absolute",
                    top: "50%",
                    left: "50%",
                    marginTop: "-12px",
                    marginLeft: "-12px",
                }}
            />
        )}
    </Button>
)
