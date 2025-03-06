// theme.ts
import { createTheme } from "@mui/material/styles"

export const paperbaseTheme = createTheme({
  palette: {
    primary: {
      light: "#00597f",
      main: "#002335",
      dark: "#000b10",
    },
    text: {
      primary: "#f5f5f5",
      secondary: "#bdbdbd",
      disabled: "#888888",
    },
  },
  typography: {
    // h5: {
    //   fontWeight: 500,
    //   fontSize: 26,
    //   letterSpacing: 0.5,
    // },
  },
  shape: {
    borderRadius: 8,
  },
  mixins: {
    toolbar: {
      minHeight: 48,
    },
  },
  components: {
    MuiTab: {
      defaultProps: {
        disableRipple: true,
      },
    },
  },
})

const extendedPaperbaseTheme = {
  ...paperbaseTheme,
  components: {
    MuiPaper: {
      styleOverrides: {
        root: {
          backgroundColor: '#587501',
          color: '#f5f5f5'
        }
      }
    },
    MuiButton: {
      styleOverrides: {
        root: {
          textTransform: "none",
          "&.Mui-disabled": {
            backgroundColor: "#555555", // Custom background for disabled state
            color: "#111", // Custom text color for disabled state
            borderStyle: "solid",
            borderColor: "#888888",
            borderWidth: "1px",
          },
        },
        contained: {
          boxShadow: "none",
          "&:active": {
            boxShadow: "none",
          },
        },
      },
    },
    MuiTabs: {
      styleOverrides: {
        root: {
          marginLeft: paperbaseTheme.spacing(1),
        },
        indicator: {
          height: 3,
          borderTopLeftRadius: 3,
          borderTopRightRadius: 3,
          backgroundColor: paperbaseTheme.palette.common.white,
        },
      },
    },
    MuiTab: {
      styleOverrides: {
        root: {
          textTransform: "none",
          margin: "0 16px",
          minWidth: 0,
          padding: 0,
          [paperbaseTheme.breakpoints.up("md")]: {
            padding: 0,
            minWidth: 0,
          },
        },
      },
    },
    MuiIconButton: {
      styleOverrides: {
        root: {
          padding: paperbaseTheme.spacing(1),
        },
      },
    },
    MuiTooltip: {
      styleOverrides: {
        tooltip: {
          borderRadius: 4,
        },
      },
    },
    MuiDivider: {
      styleOverrides: {
        root: {
          backgroundColor: "rgb(255,255,255,0.15)",
        },
      },
    },
    MuiListItemButton: {
      styleOverrides: {
        root: {
          "&.Mui-selected": {
            color: "#4fc3f7",
          },
        },
      },
    },
    MuiListItemText: {
      styleOverrides: {
        primary: {
          fontSize: 14,
          fontWeight: paperbaseTheme.typography.fontWeightMedium,
        },
      },
    },
    MuiListItemIcon: {
      styleOverrides: {
        root: {
          color: "inherit",
          minWidth: "auto",
          marginRight: paperbaseTheme.spacing(2),
          "& svg": {
            fontSize: 20,
          },
        },
      },
    },
    MuiAvatar: {
      styleOverrides: {
        root: {
          width: 32,
          height: 32,
        },
      },
    },
    MuiPanelResizeHandle: {
      styleOverrides: {
        root: {
          backgroundColor: paperbaseTheme.palette.primary.light,
          width: 4,
          '&[data-panel-group-direction="horizontal"]': {
            cursor: "col-resize",
          },
          '&[data-panel-group-direction="vertical"]': {
            height: 4,
            cursor: "row-resize",
          },
          "&:hover": {
            backgroundColor: paperbaseTheme.palette.secondary.main,
          },
          transition: "background-color 0.2s ease",
        },
      },
    },
  },
}

export default extendedPaperbaseTheme
