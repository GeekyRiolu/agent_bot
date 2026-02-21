Strategy


POST
/api/v1/strategy/parse
Parse Strategy

Parse natural language strategy into structured AST and parameters.

This endpoint converts a strategy description like: "Buy when RSI < 30, sell when RSI > 70. 5% stop loss."

Into a structured format with:

AST (entry/exit conditions)
Risk parameters (stop loss, take profit, capital)
Date configuration
Date conditions (skip months, holidays, etc.)
Stock symbols
Missing parameters that frontend should prompt for
Parameters
Try it out
Reset
No parameters

Request body

application/json
Example Value
Schema
{
  "strategy_text": "Buy when RSI is below 30, sell when RSI goes above 70. Use 5% stop loss and 15% take profit. For the stock Eternal. Skip March."
}
Responses
Curl

curl -X 'POST' \
  'https://strategy-builder-dev-61887550400.us-central1.run.app/api/v1/strategy/parse' \
  -H 'accept: application/json' \
  -H 'Content-Type: application/json' \
  -d '{
  "strategy_text": "Buy when RSI is below 30, sell when RSI goes above 70. Use 5% stop loss and 15% take profit. For the stock Eternal. Skip March."
}'
Request URL
https://strategy-builder-dev-61887550400.us-central1.run.app/api/v1/strategy/parse
Server response
Code	Details
200	
Response body
Download
{
  "success": true,
  "data": {
    "original_text": "Buy when RSI is below 30, sell when RSI goes above 70. Use 5% stop loss and 15% take profit. For the stock Eternal. Skip March.",
    "ast": {
      "type": "simple",
      "entry": [
        {
          "left": "rsi(close,14)",
          "operator": "<",
          "right": 30,
          "editable": {
            "threshold": 30,
            "min_value": 0,
            "max_value": 100,
            "step": 1,
            "indicator": "rsi",
            "params": {
              "period": 14
            }
          },
          "description": "rsi(14) is below 30"
        }
      ],
      "exit": [
        {
          "left": "rsi(close,14)",
          "operator": ">",
          "right": 70,
          "editable": {
            "threshold": 70,
            "min_value": 0,
            "max_value": 100,
            "step": 1,
            "indicator": "rsi",
            "params": {
              "period": 14
            }
          },
          "description": "rsi(14) is above 70"
        }
      ]
    },
    "risk_params": {
      "initial_capital": null,
      "position_size": null,
      "stop_loss": 5,
      "take_profit": 15
    },
    "date_config": {
      "start_date": null,
      "end_date": null,
      "is_relative": false,
      "relative_value": null
    },
    "date_conditions": [
      {
        "type": "skip_month",
        "month": "march"
      }
    ],
    "exit_after_days": null,
    "stocks": [
      "ETERNAL.NS"
    ],
    "missing_params": [
      "initial_capital",
      "position_size",
      "time_range"
    ],
    "indicators_used": [
      "rsi(close,14)"
    ],
    "available_toggles": {
      "skip_holidays": false,
      "day_of_week": null,
      "exit_after_days": null,
      "month_filter": null
    }
  }
}
Response headers
 access-control-allow-credentials: true 
 access-control-allow-origin: * 
 alt-svc: h3=":443"; ma=2592000,h3-29=":443"; ma=2592000 
 content-length: 1100 
 content-type: application/json 
 date: Sat,21 Feb 2026 08:04:47 GMT 
 server: Google Frontend 
 x-cloud-trace-context: f3118c52d572c89cf56ef192cc9262e7;o=1 
Responses
Code	Description	Links
200	
Successful Response

Media type

application/json
Controls Accept header.
Example Value
Schema
{
  "data": {
    "ast": {
      "entry": [
        {
          "description": "RSI(14) is below 30",
          "editable": {
            "indicator": "rsi",
            "max_value": 100,
            "min_value": 0,
            "params": {
              "period": 14
            },
            "step": 1,
            "threshold": 30
          },
          "left": "rsi(close,14)",
          "operator": "<",
          "right": 30
        }
      ],
      "exit": [
        {
          "description": "RSI(14) is above 70",
          "editable": {
            "indicator": "rsi",
            "max_value": 100,
            "min_value": 0,
            "params": {
              "period": 14
            },
            "step": 1,
            "threshold": 70
          },
          "left": "rsi(close,14)",
          "operator": ">",
          "right": 70
        }
      ]
    },
    "available_toggles": {
      "skip_holidays": false
    },
    "date_conditions": [
      {
        "exclude": true,
        "months": [
          "march"
        ],
        "type": "month"
      }
    ],
    "date_config": {
      "end_date": "2026-01-21",
      "is_relative": true,
      "relative_value": "last_1_year",
      "start_date": "2025-01-21"
    },
    "indicators_used": [
      "rsi(close,14)"
    ],
    "missing_params": [
      "initial_capital",
      "position_size"
    ],
    "original_text": "Buy when RSI < 30, sell when RSI > 70...",
    "risk_params": {
      "stop_loss": 5,
      "take_profit": 15
    },
    "stocks": [
      "RELIANCE.NS"
    ]
  },
  "success": true
}
No links
422	
Validation Error

Media type

application/json
Example Value
Schema
{
  "detail": [
    {
      "loc": [
        "string",
        0
      ],
      "msg": "string",
      "type": "string",
      "input": "string",
      "ctx": {}
    }
  ]
}
No links

POST
/api/v1/strategy/validate
Validate Strategy

Validate a strategy AST without running backtest.

Checks:

AST structure is valid
Indicators are recognized
Conditions are properly formed
Parameters
Try it out
No parameters

Request body

application/json
Example Value
Schema
{
  "ast": {
    "entry": [
      {
        "left": "rsi(close,14)",
        "operator": "<",
        "right": 30
      }
    ],
    "exit": [
      {
        "left": "rsi(close,14)",
        "operator": ">",
        "right": 70
      }
    ]
  }
}
Responses
Code	Description	Links
200	
Successful Response

Media type

application/json
Controls Accept header.
Example Value
Schema
{
  "data": {
    "conditions_count": {
      "entry": 1,
      "exit": 1
    },
    "errors": [],
    "indicators": [
      "rsi(close,14)"
    ],
    "valid": true,
    "warnings": []
  },
  "success": true
}
No links
422	
Validation Error

Media type

application/json
Example Value
Schema
{
  "detail": [
    {
      "loc": [
        "string",
        0
      ],
      "msg": "string",
      "type": "string",
      "input": "string",
      "ctx": {}
    }
  ]
}
No links
Backtest


POST
/api/v1/backtest/run
Run Backtest From Text


POST
/api/v1/backtest/run-config
Run Backtest With Config

Run backtest with full configuration (tweaked parameters).

Use this endpoint after parsing and letting user modify parameters. Frontend sends the modified config from the tweak UI.

Parameters
Try it out
No parameters

Request body

application/json
Example Value
Schema
{
  "ast": {
    "entry": [
      {
        "left": "rsi(close,14)",
        "operator": "<",
        "right": 25
      }
    ],
    "exit": [
      {
        "left": "rsi(close,14)",
        "operator": ">",
        "right": 75
      }
    ]
  },
  "date_conditions": [
    {
      "exclude": true,
      "months": [
        "march"
      ],
      "type": "month"
    },
    {
      "type": "skip_holiday"
    }
  ],
  "date_config": {
    "end_date": "2025-01-01",
    "start_date": "2024-01-01"
  },
  "exit_after_days": 5,
  "risk_params": {
    "initial_capital": 200000,
    "position_size": 15,
    "stop_loss": 5,
    "take_profit": 20
  },
  "stocks": [
    "RELIANCE.NS",
    "TCS.NS"
  ]
}
Responses
Code	Description	Links
200	
Successful Response

Media type

application/json
Controls Accept header.
Example Value
Schema
{
  "data": {
    "config_used": {
      "ast": {
        "entry": [
          {
            "left": "rsi(close,14)",
            "operator": "<",
            "right": 30
          }
        ],
        "exit": [
          {
            "left": "rsi(close,14)",
            "operator": ">",
            "right": 70
          }
        ]
      },
      "date_conditions": [
        {
          "exclude": true,
          "months": [
            "march"
          ],
          "type": "month"
        }
      ],
      "date_config": {
        "end_date": "2025-01-01",
        "start_date": "2024-01-01"
      },
      "stocks": [
        "RELIANCE.NS"
      ]
    },
    "results": {
      "RELIANCE.NS": {
        "equity_curve": [
          {
            "date": "2024-01-01",
            "equity": 100000
          }
        ],
        "metrics": {
          "return_pct": 12.5,
          "total_return": 25000,
          "total_trades": 8,
          "win_rate": 62.5
        },
        "stock": "RELIANCE.NS",
        "success": true,
        "trades": [
          {
            "entry_date": "2024-03-15",
            "exit_date": "2024-04-20",
            "pnl": 6800,
            "pnl_pct": 6.94
          }
        ]
      }
    },
    "summary": {
      "execution_time": 2.3,
      "failed": 0,
      "successful": 1,
      "total_stocks": 1
    }
  },
  "success": true
}
No links
422	
Validation Error

Media type

application/json
Example Value
Schema
{
  "detail": [
    {
      "loc": [
        "string",
        0
      ],
      "msg": "string",
      "type": "string",
      "input": "string",
      "ctx": {}
    }
  ]
}
